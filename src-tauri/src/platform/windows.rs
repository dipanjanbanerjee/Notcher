//! Windows 10 / 11.
//!
//! Click-through: Windows has no per-pixel input shape for a transparent web view, so
//! the window ignores the mouse entirely until the cursor is over the island. A
//! watcher polls the cursor position and flips that switch, and tells the web view
//! about enter/leave because it cannot see the pointer while ignoring it.
//!
//! The watcher only uses plain Win32 calls, which are safe from any thread and cost
//! microseconds. Tauri's window getters would each wait for the UI thread, and doing
//! that 60 times a second steals time from rendering.

use super::{emit_pointer, PlatformMode};
use crate::island::{fullscreen_on_monitor, MonitorRect, StackedWindow};
use crate::island::{hover_changed, to_window_logical, HitRegion, WINDOW_WIDTH};
use std::sync::atomic::{AtomicIsize, Ordering};
use std::time::{Duration, Instant};
use tauri::{Manager, Runtime, WebviewWindow};
use windows::core::BOOL;
use windows::Win32::{
    Foundation::{HWND, LPARAM, POINT, RECT},
    Graphics::{
        Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED},
        Gdi::{
            EnumDisplayMonitors, GetMonitorInfoW, MonitorFromWindow, HDC, HMONITOR, MONITORINFO,
            MONITOR_DEFAULTTONEAREST,
        },
    },
    UI::{
        HiDpi::{GetDpiForMonitor, GetDpiForWindow, MDT_EFFECTIVE_DPI},
        Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON, VK_MBUTTON, VK_RBUTTON},
        Shell::{
            SHQueryUserNotificationState, QUNS_BUSY, QUNS_PRESENTATION_MODE,
            QUNS_RUNNING_D3D_FULL_SCREEN,
        },
        WindowsAndMessaging::{
            GetClassNameW, GetCursorPos, GetForegroundWindow, GetSystemMetrics, GetTopWindow,
            GetWindow, GetWindowLongW, GetWindowRect, IsIconic, IsWindowVisible, IsZoomed,
            GWL_EXSTYLE, GWL_STYLE, GW_HWNDNEXT, GW_HWNDPREV, GW_OWNER, SM_CXDRAG, SM_CYDRAG,
            WS_CAPTION, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
        },
    },
};

/// The island's window handle, stored as an integer so the watcher thread can use it.
static ISLAND_HWND: AtomicIsize = AtomicIsize::new(0);

fn island_hwnd() -> Option<HWND> {
    match ISLAND_HWND.load(Ordering::Relaxed) {
        0 => None,
        raw => Some(HWND(raw as *mut _)),
    }
}

pub fn prepare_island<R: Runtime>(window: &WebviewWindow<R>) -> PlatformMode {
    if let Some(hwnd) = hwnd_of(window) {
        ISLAND_HWND.store(hwnd.0 as isize, Ordering::Relaxed);
    }
    // Start fully click-through; the watcher enables input once the cursor is on the island.
    let _ = window.set_ignore_cursor_events(true);
    // Runs on the main thread (from setup), which the drop catcher must live on.
    super::win_drop::create_catcher(window.app_handle());
    PlatformMode::default()
}

/// The watcher reads the region from shared state, so nothing to do here.
pub fn apply_hit_region<R: Runtime>(_window: &WebviewWindow<R>, _region: HitRegion) {}

/// Windows cannot give the web view per-pixel input, so the watcher polls the cursor.
pub const POLLS_POINTER: bool = true;

/// Cursor position in physical screen pixels.
pub fn cursor_position<R: Runtime>(_window: &WebviewWindow<R>) -> Option<(f64, f64)> {
    let mut p = POINT::default();
    unsafe { GetCursorPos(&mut p) }.ok()?;
    Some((p.x as f64, p.y as f64))
}

pub fn monitors<R: Runtime>(_window: &WebviewWindow<R>) -> Vec<MonitorRect> {
    unsafe extern "system" fn collect(
        monitor: HMONITOR,
        _hdc: HDC,
        _rect: *mut RECT,
        data: LPARAM,
    ) -> BOOL {
        let out = &mut *(data.0 as *mut Vec<MonitorRect>);
        if let Some(rect) = monitor_rect(monitor) {
            out.push(rect);
        }
        BOOL(1)
    }
    let mut out: Vec<MonitorRect> = Vec::new();
    unsafe {
        let _ = EnumDisplayMonitors(
            None,
            None,
            Some(collect),
            LPARAM(&mut out as *mut Vec<MonitorRect> as isize),
        );
    }
    out
}

/// While something is being dragged, the drop catcher covers this area around the top
/// centre of the island window (and the island itself when it is bigger), in logical
/// pixels. That way files can be dropped on the island even when it is tucked into its
/// thin tab or resting small.
const DROP_ZONE: HitRegion = HitRegion {
    x: (WINDOW_WIDTH - 380.0) / 2.0,
    y: 0.0,
    width: 380.0,
    height: 60.0,
};

/// The catcher comes out once a drag is this close to the drop zone (logical pixels),
/// so it is already in place when the files arrive over the notch.
const DROP_APPROACH: f64 = 160.0;

/// After the button is released, the catcher stays a moment so Windows can finish
/// handing over the drop.
const CATCHER_LINGER: Duration = Duration::from_millis(400);

/// A mouse button is held down: something is being dragged (a window by its title
/// bar, a widget, files from Explorer) or selected.
fn button_down() -> bool {
    [VK_LBUTTON, VK_RBUTTON, VK_MBUTTON]
        .iter()
        .any(|key| unsafe { GetAsyncKeyState(key.0 as i32) } as u16 & 0x8000 != 0)
}

/// How far the pointer must move with a button held before it counts as a drag, in
/// physical pixels (Windows' own drag threshold, at least 4).
fn drag_threshold() -> (i32, i32) {
    unsafe {
        (
            GetSystemMetrics(SM_CXDRAG).max(4),
            GetSystemMetrics(SM_CYDRAG).max(4),
        )
    }
}

/// How many pointer checks pass between looks for new web view windows (about 0.4 s).
const DROP_TARGET_EVERY: u32 = 25;

/// The drop catcher's state during one press of a mouse button.
#[derive(Default)]
struct DragWatch {
    /// Where the button went down, in physical pixels.
    pressed_at: Option<(i32, i32)>,
    /// The button went down on the island itself.
    pressed_on_island: bool,
    /// The catcher is over the notch, at this physical rectangle.
    shown: Option<RECT>,
    /// When the button was released while the catcher was out.
    released: Option<Instant>,
    /// Where the pointer was when the button was released.
    released_at: (i32, i32),
    /// Drags Notcher's handlers had seen when the catcher came out.
    enters_before: u32,
}

#[derive(Default)]
pub struct PointerTracker {
    inside: Option<bool>,
    /// Whether the window currently takes mouse input.
    accepting: Option<bool>,
    /// Cursor position on the previous check, in physical pixels.
    last_cursor: Option<(i32, i32)>,
    drag: DragWatch,
    /// Summary of the island's inner windows when drop handlers were last put on them.
    children: Option<u64>,
    checks: u32,
}

/// `region` (logical island-window pixels) on screen, in physical pixels.
fn to_screen(region: HitRegion, origin: (i32, i32), scale: f64) -> RECT {
    RECT {
        left: origin.0 + (region.x * scale).floor() as i32,
        top: origin.1 + (region.y * scale).floor() as i32,
        right: origin.0 + ((region.x + region.width) * scale).ceil() as i32,
        bottom: origin.1 + ((region.y + region.height) * scale).ceil() as i32,
    }
}

fn union(a: HitRegion, b: HitRegion) -> HitRegion {
    if b.width <= 0.0 || b.height <= 0.0 {
        return a;
    }
    let x = a.x.min(b.x);
    let y = a.y.min(b.y);
    HitRegion {
        x,
        y,
        width: (a.x + a.width).max(b.x + b.width) - x,
        height: (a.y + a.height).max(b.y + b.height) - y,
    }
}

fn inflate(r: HitRegion, by: f64) -> HitRegion {
    HitRegion {
        x: r.x - by,
        y: r.y,
        width: r.width + by * 2.0,
        height: r.height + by,
    }
}

impl PointerTracker {
    pub fn update<R: Runtime>(&mut self, window: &WebviewWindow<R>, region: HitRegion) {
        let Some(hwnd) = island_hwnd() else { return };
        self.checks = self.checks.wrapping_add(1);
        if self.checks % DROP_TARGET_EVERY == 1 {
            self.keep_drop_targets(window, hwnd, false);
        }
        let mut cursor = POINT::default();
        let mut rect = RECT::default();
        if unsafe { GetCursorPos(&mut cursor) }.is_err()
            || unsafe { GetWindowRect(hwnd, &mut rect) }.is_err()
        {
            return;
        }
        let dpi = unsafe { GetDpiForWindow(hwnd) };
        let scale = if dpi > 0 { dpi as f64 / 96.0 } else { 1.0 };
        let (x, y) = to_window_logical(
            (cursor.x as f64, cursor.y as f64),
            (rect.left, rect.top),
            scale,
        );
        let moved = self.last_cursor != Some((cursor.x, cursor.y));
        self.last_cursor = Some((cursor.x, cursor.y));
        let inside = region.contains(x, y);
        let held = button_down();
        self.watch_drag(
            window,
            hwnd,
            held,
            (cursor.x, cursor.y),
            (x, y),
            (rect.left, rect.top),
            scale,
            region,
        );
        if self.accepting != Some(inside) {
            self.accepting = Some(inside);
            let _ = window.set_ignore_cursor_events(!inside);
        }
        // While a button is held, the island neither opens nor closes on hover: you are
        // moving a window or a widget, not reaching for Notcher. A file drag still opens
        // it, through the drop handler. Once the button is released, hover works again.
        if held {
            return;
        }
        if hover_changed(self.inside, inside, moved) {
            self.inside = Some(inside);
            emit_pointer(window, inside);
        }
    }

    /// Brings the drop catcher out while something is dragged toward the notch, keeps
    /// it over the island as the island grows, and takes it away once the drag ends.
    #[allow(clippy::too_many_arguments)]
    fn watch_drag<R: Runtime>(
        &mut self,
        window: &WebviewWindow<R>,
        hwnd: HWND,
        held: bool,
        cursor: (i32, i32),
        logical: (f64, f64),
        origin: (i32, i32),
        scale: f64,
        region: HitRegion,
    ) {
        let d = &mut self.drag;
        if !held {
            d.pressed_at = None;
            let Some(shown) = d.shown else { return };
            let released = *d.released.get_or_insert_with(|| {
                d.released_at = cursor;
                Instant::now()
            });
            if released.elapsed() < CATCHER_LINGER {
                return;
            }
            super::win_drop::hide_catcher();
            d.shown = None;
            d.released = None;
            let (rx, ry) = d.released_at;
            let over = rx >= shown.left && rx < shown.right && ry >= shown.top && ry < shown.bottom;
            if over && super::win_drop::drag_enters() == d.enters_before {
                log::warn!(
                    "file drop: a drag was released over the notch at ({rx}, {ry}) px but Windows delivered nothing to Notcher. \
                     It was not a file drag, came from an app running as administrator, or another window sat on top"
                );
            } else {
                log::info!("file drop: drag over, drop catcher put away");
            }
            return;
        }
        d.released = None;
        if d.pressed_at.is_none() {
            d.pressed_at = Some(cursor);
            // A press on the island itself starts a drag inside it (a Home widget, a
            // slider, selected text): the web view handles that, not the catcher.
            d.pressed_on_island = region.contains(logical.0, logical.1);
        }
        let start = d.pressed_at.unwrap_or(cursor);
        let (tx, ty) = drag_threshold();
        let dragging = (cursor.0 - start.0).abs() >= tx || (cursor.1 - start.1).abs() >= ty;
        let zone = union(DROP_ZONE, region);
        let near = dragging
            && !d.pressed_on_island
            && inflate(zone, DROP_APPROACH).contains(logical.0, logical.1);
        if !near {
            if d.shown.take().is_some() {
                super::win_drop::hide_catcher();
                log::info!("file drop: the drag moved away from the notch");
            }
            return;
        }
        let target = to_screen(zone, origin, scale);
        if d.shown == Some(target) {
            return;
        }
        if d.shown.is_none() {
            d.enters_before = super::win_drop::drag_enters();
            log::info!(
                "file drop: drag approaching the notch at ({}, {}) px, scale {scale}; drop catcher at ({}, {})-({}, {})",
                cursor.0,
                cursor.1,
                target.left,
                target.top,
                target.right,
                target.bottom
            );
            if super::win_drop::catcher().is_none() {
                log::warn!("file drop: no drop catcher; relying on the web view's handlers");
            }
            // Make sure the web view's own handlers are in place too.
            self.keep_drop_targets(window, hwnd, true);
        }
        let d = &mut self.drag;
        d.shown = Some(target);
        super::win_drop::show_catcher(target);
    }

    /// Puts drop handlers on the island's windows when WebView2 has added or replaced
    /// one since last time (or always, with `force`).
    fn keep_drop_targets<R: Runtime>(
        &mut self,
        window: &WebviewWindow<R>,
        hwnd: HWND,
        force: bool,
    ) {
        let now = super::win_drop::fingerprint(hwnd);
        if !force && self.children == Some(now) {
            return;
        }
        self.children = Some(now);
        let app = window.app_handle().clone();
        let raw = hwnd.0 as isize;
        let _ = window.run_on_main_thread(move || {
            super::win_drop::ensure_targets(&app, HWND(raw as *mut _));
        });
    }

    /// Forces the next update to re-apply the click-through state (e.g. after a move).
    pub fn invalidate(&mut self) {
        if self.drag.shown.take().is_some() {
            super::win_drop::hide_catcher();
        }
        self.inside = None;
        self.accepting = None;
        self.last_cursor = None;
    }
}

fn hwnd_of<R: Runtime>(window: &WebviewWindow<R>) -> Option<HWND> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    match window.window_handle().ok()?.as_raw() {
        RawWindowHandle::Win32(h) => Some(HWND(h.hwnd.get() as *mut _)),
        _ => None,
    }
}

fn class_name(hwnd: HWND) -> String {
    let mut buf = [0u16; 128];
    let len = unsafe { GetClassNameW(hwnd, &mut buf) } as usize;
    String::from_utf16_lossy(&buf[..len.min(buf.len())])
}

/// The desktop, a taskbar, or one of the shell flyouts that open from the taskbar.
fn is_shell_window(hwnd: HWND) -> bool {
    crate::island::is_shell_class(&class_name(hwnd))
}

fn monitor_rect(monitor: HMONITOR) -> Option<MonitorRect> {
    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    if !unsafe { GetMonitorInfoW(monitor, &mut info) }.as_bool() {
        return None;
    }
    let r = info.rcMonitor;
    let (mut dpi_x, mut dpi_y) = (96u32, 96u32);
    let _ = unsafe { GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y) };
    Some(MonitorRect {
        x: r.left,
        y: r.top,
        width: (r.right - r.left).max(0) as u32,
        height: (r.bottom - r.top).max(0) as u32,
        scale: dpi_x.max(1) as f64 / 96.0,
    })
}

/// Windows that should not count as "the app on top": invisible, minimised, cloaked
/// (hidden UWP apps and other virtual desktops), click-through overlays, tool windows
/// and the shell itself.
fn is_candidate(hwnd: HWND, own: HWND) -> bool {
    unsafe {
        if hwnd == own || !IsWindowVisible(hwnd).as_bool() || IsIconic(hwnd).as_bool() {
            return false;
        }
        let ex = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
        if ex & (WS_EX_TOOLWINDOW.0 | WS_EX_TRANSPARENT.0) != 0 {
            return false;
        }
        // Menus, dropdowns, tooltips and dialogs belong to an app window. They flash
        // above a maximized window all the time and must not count as "the app on top",
        // or the island would pop in and out while you work.
        if GetWindow(hwnd, GW_OWNER).is_ok_and(|owner| !owner.is_invalid()) {
            return false;
        }
        let style = GetWindowLongW(hwnd, GWL_STYLE) as u32;
        if style & WS_POPUP.0 != 0 && style & WS_CAPTION.0 == 0 && is_small(hwnd) {
            return false;
        }
        let mut cloaked: u32 = 0;
        let _ = DwmGetWindowAttribute(
            hwnd,
            DWMWA_CLOAKED,
            &mut cloaked as *mut u32 as *mut _,
            std::mem::size_of::<u32>() as u32,
        );
        cloaked == 0 && !is_shell_window(hwnd)
    }
}

/// Borderless popups smaller than about a third of a typical screen (menus, toasts,
/// autocomplete lists). Borderless full-screen windows are much larger.
fn is_small(hwnd: HWND) -> bool {
    let mut r = RECT::default();
    if unsafe { GetWindowRect(hwnd, &mut r) }.is_err() {
        return true;
    }
    let (w, h) = ((r.right - r.left) as i64, (r.bottom - r.top) as i64);
    w * h < 800 * 600
}

/// True when a full-screen app, game or presentation is on top on the island's monitor.
/// Walks the z-order instead of only checking the focused window, so the island stays
/// hidden even after it was clicked itself.
/// With `include_maximized`, a maximized app window counts too, so the island tucks
/// away while you work in a maximized window.
pub fn fullscreen_app_active<R: Runtime>(
    _window: &WebviewWindow<R>,
    _monitor: &MonitorRect,
    include_maximized: bool,
) -> bool {
    let Some(own) = island_hwnd() else {
        return false;
    };
    // Ask Windows directly so the rectangle matches GetWindowRect exactly.
    let Some(monitor) = monitor_rect(unsafe { MonitorFromWindow(own, MONITOR_DEFAULTTONEAREST) })
    else {
        return false;
    };
    // The shell itself tracks full-screen apps (browser F11, video players, games and
    // presentations) for the foreground window, to keep notifications quiet.
    if let Ok(state) = unsafe { SHQueryUserNotificationState() } {
        if state == QUNS_BUSY
            || state == QUNS_RUNNING_D3D_FULL_SCREEN
            || state == QUNS_PRESENTATION_MODE
        {
            let foreground = unsafe { GetForegroundWindow() };
            // The desktop itself is "full screen" as far as the shell is concerned, so
            // clicking the wallpaper or the taskbar must not count.
            if foreground != own
                && !is_shell_window(foreground)
                && unsafe { MonitorFromWindow(foreground, MONITOR_DEFAULTTONEAREST) }
                    == unsafe { MonitorFromWindow(own, MONITOR_DEFAULTTONEAREST) }
            {
                return true;
            }
        }
    }
    // Otherwise walk the z-order from the top, looking past small always-on-top
    // overlays, until the first real app window on this monitor.
    let mut windows = Vec::new();
    let mut hwnd = unsafe { GetTopWindow(None) }.unwrap_or_default();
    let mut guard = 0;
    while !hwnd.is_invalid() && guard < 1024 {
        guard += 1;
        if is_candidate(hwnd, own) {
            let mut rect = RECT::default();
            if unsafe { GetWindowRect(hwnd, &mut rect) }.is_ok() {
                let ex = unsafe { GetWindowLongW(hwnd, GWL_EXSTYLE) } as u32;
                let w = StackedWindow {
                    x: rect.left,
                    y: rect.top,
                    width: (rect.right - rect.left).max(0) as u32,
                    height: (rect.bottom - rect.top).max(0) as u32,
                    // Maximized windows are treated like full-screen ones when asked.
                    fullscreen: include_maximized && unsafe { IsZoomed(hwnd) }.as_bool(),
                    overlay: ex & WS_EX_TOPMOST.0 != 0,
                };
                let on_monitor = monitor.contains(
                    w.x as f64 + w.width as f64 / 2.0,
                    w.y as f64 + w.height as f64 / 2.0,
                );
                windows.push(w);
                if on_monitor && !w.overlay {
                    break;
                }
            }
        }
        hwnd = unsafe { GetWindow(hwnd, GW_HWNDNEXT) }.unwrap_or_default();
    }
    fullscreen_on_monitor(&windows, &monitor)
}

/// Why the taskbar `taskbar` can't be seen right now, or `None` when it can: it is
/// hidden, auto-hidden off the edge of the screen, or covered by a full-screen window
/// (a video, a game, a browser in F11 or a full-screen YouTube video).
///
/// Shell flyouts that open from the taskbar (Start, Search, Task View…) are ignored:
/// some of them fill the whole monitor while open, and counting them made the meter
/// vanish whenever the taskbar was clicked.
pub(crate) fn taskbar_hidden_reason(taskbar: HWND) -> Option<String> {
    unsafe {
        if !IsWindowVisible(taskbar).as_bool() {
            return Some("the taskbar window is hidden".into());
        }
        let mut cloaked: u32 = 0;
        let _ = DwmGetWindowAttribute(
            taskbar,
            DWMWA_CLOAKED,
            &mut cloaked as *mut u32 as *mut _,
            std::mem::size_of::<u32>() as u32,
        );
        if cloaked != 0 {
            return Some("the taskbar is cloaked".into());
        }
        let mut bar = RECT::default();
        if GetWindowRect(taskbar, &mut bar).is_err() {
            return Some("the taskbar has no position".into());
        }
        let monitor_handle = MonitorFromWindow(taskbar, MONITOR_DEFAULTTONEAREST);
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if !GetMonitorInfoW(monitor_handle, &mut info).as_bool() {
            return None;
        }
        let m = info.rcMonitor;
        if !crate::netspeed::mostly_on_screen(
            (bar.left, bar.top, bar.right, bar.bottom),
            (m.left, m.top, m.right, m.bottom),
        ) {
            return Some("the taskbar is auto-hidden".into());
        }
        let own = island_hwnd().unwrap_or_default();
        let covers = |hwnd: HWND| {
            let mut r = RECT::default();
            GetWindowRect(hwnd, &mut r).is_ok()
                && r.left <= m.left
                && r.top <= m.top
                && r.right >= m.right
                && r.bottom >= m.bottom
        };
        // The app in front, on the taskbar's monitor, filling that monitor. Browsers
        // showing a full-screen video make the shell report "busy" rather than a D3D
        // full-screen app, so the window's own rectangle is what decides. A video
        // player's full-screen window is often owned by its main window, which the
        // walk below skips, so this has to come first.
        let foreground = GetForegroundWindow();
        let fg_usable = !foreground.is_invalid()
            && foreground != own
            && IsWindowVisible(foreground).as_bool()
            && !is_shell_window(foreground)
            && MonitorFromWindow(foreground, MONITOR_DEFAULTTONEAREST) == monitor_handle;
        if fg_usable && covers(foreground) {
            return Some(format!(
                "full-screen window in front ({})",
                class_name(foreground)
            ));
        }
        // Exclusive full-screen games and presentations, as the shell sees them.
        if fg_usable {
            if let Ok(state) = SHQueryUserNotificationState() {
                if state == QUNS_RUNNING_D3D_FULL_SCREEN || state == QUNS_PRESENTATION_MODE {
                    return Some(format!(
                        "full-screen game or presentation ({})",
                        class_name(foreground)
                    ));
                }
            }
        }
        // Anything else stacked above the taskbar that fills its monitor.
        let mut hwnd = GetTopWindow(None).unwrap_or_default();
        let mut guard = 0;
        while !hwnd.is_invalid() && hwnd != taskbar && guard < 1024 {
            guard += 1;
            if is_candidate(hwnd, own) && covers(hwnd) {
                return Some(format!(
                    "full-screen window above the taskbar ({})",
                    class_name(hwnd)
                ));
            }
            hwnd = GetWindow(hwnd, GW_HWNDNEXT).unwrap_or_default();
        }
        None
    }
}

/// True when `upper` is above `lower` in the z-order.
pub(crate) fn is_above(upper: HWND, lower: HWND) -> bool {
    let mut hwnd = unsafe { GetWindow(lower, GW_HWNDPREV) }.unwrap_or_default();
    let mut guard = 0;
    while !hwnd.is_invalid() && guard < 1024 {
        if hwnd == upper {
            return true;
        }
        guard += 1;
        hwnd = unsafe { GetWindow(hwnd, GW_HWNDPREV) }.unwrap_or_default();
    }
    false
}
