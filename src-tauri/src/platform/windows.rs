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
use crate::island::{to_window_logical, HitRegion, WINDOW_WIDTH};
use std::sync::atomic::{AtomicIsize, Ordering};
use tauri::{Runtime, WebviewWindow};
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
        Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON, VK_RBUTTON},
        Shell::{
            SHQueryUserNotificationState, QUNS_BUSY, QUNS_PRESENTATION_MODE,
            QUNS_RUNNING_D3D_FULL_SCREEN,
        },
        WindowsAndMessaging::{
            GetClassNameW, GetCursorPos, GetForegroundWindow, GetSystemMetrics, GetTopWindow,
            GetWindow, GetWindowLongW, GetWindowRect, IsIconic, IsWindowVisible, IsZoomed,
            GWL_EXSTYLE, GWL_STYLE, GW_HWNDNEXT, GW_OWNER, SM_SWAPBUTTON, WS_CAPTION,
            WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
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

/// The primary mouse button is held down (a drag may be in progress).
fn primary_button_down() -> bool {
    let swapped = unsafe { GetSystemMetrics(SM_SWAPBUTTON) } != 0;
    let key = if swapped { VK_RBUTTON } else { VK_LBUTTON };
    let state = unsafe { GetAsyncKeyState(key.0 as i32) };
    state as u16 & 0x8000 != 0
}

#[derive(Default)]
pub struct PointerTracker {
    inside: Option<bool>,
    /// Whether the window currently takes mouse input (and therefore drops).
    accepting: Option<bool>,
}

impl PointerTracker {
    pub fn update<R: Runtime>(&mut self, window: &WebviewWindow<R>, region: HitRegion) {
        let Some(hwnd) = island_hwnd() else { return };
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
        let inside = region.contains(x, y);
        // Dragging files from Explorer toward the top of the screen: stop being
        // click-through there so Windows offers us the drop. This does not count as
        // hovering, so the island doesn't open until the files actually arrive over it.
        let drag_target = !inside && DROP_ZONE.contains(x, y) && primary_button_down();
        let accepting = inside || drag_target;
        if self.accepting != Some(accepting) {
            self.accepting = Some(accepting);
            let _ = window.set_ignore_cursor_events(!accepting);
        }
        if self.inside != Some(inside) {
            self.inside = Some(inside);
            emit_pointer(window, inside);
        }
    }

    /// Forces the next update to re-apply the click-through state (e.g. after a move).
    pub fn invalidate(&mut self) {
        self.inside = None;
        self.accepting = None;
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
            if foreground != own
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
