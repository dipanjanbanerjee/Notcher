//! Windows 10 / 11.
//!
//! Click-through: Windows has no per-pixel input shape for a transparent web view, so
//! the window ignores the mouse entirely until the cursor is over the island. A
//! watcher polls the cursor position and flips that switch, and tells the web view
//! about enter/leave because it cannot see the pointer while ignoring it.

use super::{emit_pointer, PlatformMode};
use crate::island::{to_window_logical, HitRegion};
use tauri::{Runtime, WebviewWindow};
use windows::Win32::{
    Foundation::{HWND, RECT},
    Graphics::Gdi::{GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST},
    UI::{
        Shell::{
            SHQueryUserNotificationState, QUNS_BUSY, QUNS_PRESENTATION_MODE,
            QUNS_RUNNING_D3D_FULL_SCREEN,
        },
        WindowsAndMessaging::{GetClassNameW, GetForegroundWindow, GetWindowRect},
    },
};

pub fn prepare_island<R: Runtime>(window: &WebviewWindow<R>) -> PlatformMode {
    // Start fully click-through; the watcher enables input once the cursor is on the island.
    let _ = window.set_ignore_cursor_events(true);
    PlatformMode::default()
}

/// The watcher reads the region from shared state, so nothing to do here.
pub fn apply_hit_region<R: Runtime>(_window: &WebviewWindow<R>, _region: HitRegion) {}

#[derive(Default)]
pub struct PointerTracker {
    inside: Option<bool>,
}

impl PointerTracker {
    pub fn update<R: Runtime>(&mut self, window: &WebviewWindow<R>, region: HitRegion) {
        let (Ok(cursor), Ok(origin), Ok(scale)) = (
            window.cursor_position(),
            window.outer_position(),
            window.scale_factor(),
        ) else {
            return;
        };
        let (x, y) = to_window_logical((cursor.x, cursor.y), (origin.x, origin.y), scale);
        let inside = region.contains(x, y);
        if self.inside != Some(inside) {
            self.inside = Some(inside);
            let _ = window.set_ignore_cursor_events(!inside);
            emit_pointer(window, inside);
        }
    }

    /// Forces the next update to re-apply the click-through state (e.g. after a move).
    pub fn invalidate(&mut self) {
        self.inside = None;
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

/// True when a full-screen app, game or presentation is in front on the island's monitor.
pub fn fullscreen_app_active<R: Runtime>(window: &WebviewWindow<R>) -> bool {
    let Some(own) = hwnd_of(window) else {
        return false;
    };
    unsafe {
        let foreground = GetForegroundWindow();
        if foreground.is_invalid() || foreground == own || is_shell_window(foreground) {
            return false;
        }
        let island_monitor = MonitorFromWindow(own, MONITOR_DEFAULTTONEAREST);
        let app_monitor = MonitorFromWindow(foreground, MONITOR_DEFAULTTONEAREST);
        if island_monitor != app_monitor {
            return false;
        }
        if let Ok(state) = SHQueryUserNotificationState() {
            if state == QUNS_BUSY
                || state == QUNS_RUNNING_D3D_FULL_SCREEN
                || state == QUNS_PRESENTATION_MODE
            {
                return true;
            }
        }
        let mut rect = RECT::default();
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if GetWindowRect(foreground, &mut rect).is_err()
            || !GetMonitorInfoW(app_monitor, &mut info).as_bool()
        {
            return false;
        }
        let m = info.rcMonitor;
        rect.left <= m.left && rect.top <= m.top && rect.right >= m.right && rect.bottom >= m.bottom
    }
}
