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
            GetClassNameW, GetCursorPos, GetForegroundWindow, GetTopWindow, GetWindow,
            GetWindowLongW, GetWindowRect, IsIconic, IsWindowVisible, IsZoomed, GWL_EXSTYLE,
            GWL_STYLE, GW_HWNDNEXT, GW_HWNDPREV, GW_OWNER, WS_CAPTION, WS_EX_TOOLWINDOW,
            WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
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

/// While something is being dragged, the island accepts drops from this area around
/// the top centre of its window, in logical pixels. That way files can be dropped on
/// the island even when it is tucked into its thin tab or resting small.
const DROP_ZONE: HitRegion = HitRegion {
    x: (WINDOW_WIDTH - 380.0) / 2.0,
    y: 0.0,
    width: 380.0,
    height: 60.0,
};

/// A mouse button is held down: something is being dragged (a window by its title
/// bar, a widget, files from Explorer) or selected.
fn button_down() -> bool {
    [VK_LBUTTON, VK_RBUTTON, VK_MBUTTON]
        .iter()
        .any(|key| unsafe { GetAsyncKeyState(key.0 as i32) } as u16 & 0x8000 != 0)
}

/// How many pointer checks pass between looks for new web view windows (about 0.4 s).
const DROP_TARGET_EVERY: u32 = 25;

#[derive(Default)]
pub struct PointerTracker {
    inside: Option<bool>,
    /// Whether the window currently takes mouse input (and therefore drops).
    accepting: Option<bool>,
    /// Cursor position on the previous check, in physical pixels.
    last_cursor: Option<(i32, i32)>,
    /// A drag is near the island, so it is taking drops.
    drag_near: bool,
    /// Summary of the island's inner windows when drop handlers were last put on them.
    children: Option<u64>,
    checks: u32,
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
        // Dragging toward the top of the screen: stop being click-through there so
        // Windows offers us a file drop. This does not count as hovering; the island
        // opens only when files actually arrive over it (the drop handler says so).
        let drag_near = held && (inside || DROP_ZONE.contains(x, y));
        if drag_near != self.drag_near {
            self.drag_near = drag_near;
            if drag_near {
                log::debug!(
                    "file drop: drag near the island at ({}, {}) px, scale {scale}",
                    cursor.x,
                    cursor.y
                );
                // Make sure the handlers are in place before the files arrive.
                self.keep_drop_targets(window, hwnd, true);
            }
        }
        let accepting = inside || drag_near;
        if self.accepting != Some(accepting) {
            self.accepting = Some(accepting);
            let _ = window.set_ignore_cursor_events(!accepting);
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

fn is_shell_window(hwnd: HWND) -> bool {
    let mut buf = [0u16; 64];
    let len = unsafe { GetClassNameW(hwnd, &mut buf) } as usize;
    let class = String::from_utf16_lossy(&buf[..len.min(buf.len())]);
    matches!(
        class.as_str(),
        "Progman" | "WorkerW" | "Shell_TrayWnd" | "Shell_SecondaryTrayWnd"
    )
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

/// True when the taskbar `taskbar` can't be seen: hidden, auto-hidden off the edge of
/// the screen, or covered by a full-screen window (a video, a game, a browser in F11).
///
/// It walks the z-order from the top down to the taskbar, so it answers the question
/// "is anything full-screen in front of the taskbar on its monitor?" directly, the same
/// way the island decides to tuck away, instead of asking the shell whether a
/// full-screen app is running somewhere.
pub(crate) fn taskbar_hidden(taskbar: HWND) -> bool {
    unsafe {
        if !IsWindowVisible(taskbar).as_bool() {
            return true;
        }
        let mut cloaked: u32 = 0;
        let _ = DwmGetWindowAttribute(
            taskbar,
            DWMWA_CLOAKED,
            &mut cloaked as *mut u32 as *mut _,
            std::mem::size_of::<u32>() as u32,
        );
        if cloaked != 0 {
            return true;
        }
        let mut bar = RECT::default();
        if GetWindowRect(taskbar, &mut bar).is_err() {
            return true;
        }
        let monitor_handle = MonitorFromWindow(taskbar, MONITOR_DEFAULTTONEAREST);
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if !GetMonitorInfoW(monitor_handle, &mut info).as_bool() {
            return false;
        }
        let m = info.rcMonitor;
        if !crate::netspeed::mostly_on_screen(
            (bar.left, bar.top, bar.right, bar.bottom),
            (m.left, m.top, m.right, m.bottom),
        ) {
            return true;
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
        // The app in front filling the taskbar's screen: a video player's full-screen
        // window is often owned by its main window, which the walk below skips.
        let foreground = GetForegroundWindow();
        if !foreground.is_invalid()
            && foreground != own
            && !is_shell_window(foreground)
            && IsWindowVisible(foreground).as_bool()
            && covers(foreground)
        {
            return true;
        }
        let mut hwnd = GetTopWindow(None).unwrap_or_default();
        let mut guard = 0;
        while !hwnd.is_invalid() && hwnd != taskbar && guard < 1024 {
            guard += 1;
            if is_candidate(hwnd, own) && covers(hwnd) {
                return true;
            }
            hwnd = GetWindow(hwnd, GW_HWNDNEXT).unwrap_or_default();
        }
        false
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
