//! A small internet speed meter on the Windows taskbar, next to the tray icons.
//!
//! It is its own tiny transparent window that sits over the taskbar, ignores the
//! mouse, and is re-raised every second (clicking the taskbar would otherwise cover
//! it). Speeds come from the vitals sampler, which already measures them.

use serde::Serialize;

#[cfg_attr(not(windows), allow(dead_code))]
pub const UPDATE_EVENT: &str = "netspeed://update";
#[cfg_attr(not(windows), allow(dead_code))]
pub const LABEL: &str = "netspeed";

#[cfg_attr(not(windows), allow(dead_code))]
#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Speed {
    /// Bytes per second.
    pub rx: u64,
    pub tx: u64,
    /// The taskbar is light, so the text should be dark.
    pub light_taskbar: bool,
}

/// Where the meter goes, in physical pixels: just left of the tray on a horizontal
/// taskbar. `None` for a vertical or hidden taskbar.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn placement(
    taskbar: (i32, i32, i32, i32),
    tray_left: Option<i32>,
    width: i32,
) -> Option<(i32, i32, i32, i32)> {
    let (left, top, right, bottom) = taskbar;
    let (w, h) = (right - left, bottom - top);
    if w <= 0 || h <= 0 || h > w {
        return None;
    }
    let anchor = tray_left
        .filter(|x| *x > left + width && *x <= right)
        .unwrap_or(right - (w / 6).max(width * 3));
    Some((anchor - width - 4, top, width, h))
}

pub fn start<R: tauri::Runtime>(app: tauri::AppHandle<R>) {
    #[cfg(windows)]
    platform::spawn(app);
    #[cfg(not(windows))]
    let _ = app;
}

#[cfg(windows)]
mod platform {
    use super::{placement, Speed, LABEL, UPDATE_EVENT};
    use crate::settings;
    use std::time::Duration;
    use tauri::{AppHandle, Emitter, Manager, Runtime, WebviewUrl, WebviewWindowBuilder};
    use windows::core::w;
    use windows::Win32::{
        Foundation::{HWND, RECT},
        System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD},
        UI::{
            HiDpi::GetDpiForWindow,
            Shell::{
                SHQueryUserNotificationState, QUNS_BUSY, QUNS_PRESENTATION_MODE,
                QUNS_RUNNING_D3D_FULL_SCREEN,
            },
            WindowsAndMessaging::{
                FindWindowExW, FindWindowW, GetWindowRect, SetWindowPos, HWND_TOPMOST,
                SWP_NOACTIVATE, SWP_SHOWWINDOW,
            },
        },
    };

    /// Logical width of the meter.
    const WIDTH: f64 = 78.0;

    pub fn spawn<R: Runtime>(app: AppHandle<R>) {
        let spawned = std::thread::Builder::new()
            .name("notcher-netspeed".into())
            .spawn(move || loop {
                tick(&app);
                std::thread::sleep(Duration::from_secs(1));
            });
        if let Err(err) = spawned {
            log::error!("could not start taskbar speed meter: {err}");
        }
    }

    fn tick<R: Runtime>(app: &AppHandle<R>) {
        let existing = app.get_webview_window(LABEL);
        if !settings::flag(app, "taskbarSpeed", true) {
            if let Some(w) = existing {
                let _ = w.destroy();
            }
            return;
        }
        let window = match existing {
            Some(w) => w,
            None => match create(app) {
                Some(w) => w,
                None => return,
            },
        };
        let Some(hwnd) = hwnd_of(&window) else { return };
        match target_rect() {
            Some((x, y, w, h)) if !fullscreen_app() => unsafe {
                // Re-raise above the taskbar every second; clicking the taskbar brings it
                // back on top of us otherwise.
                let _ = SetWindowPos(
                    hwnd,
                    Some(HWND_TOPMOST),
                    x,
                    y,
                    w,
                    h,
                    SWP_NOACTIVATE | SWP_SHOWWINDOW,
                );
            },
            _ => {
                let _ = window.hide();
                return;
            }
        }
        if let Some(v) = crate::vitals::vitals_snapshot() {
            let speed = Speed {
                rx: v.net_rx,
                tx: v.net_tx,
                light_taskbar: light_taskbar(),
            };
            let _ = app.emit_to(LABEL, UPDATE_EVENT, speed);
        }
    }

    fn create<R: Runtime>(app: &AppHandle<R>) -> Option<tauri::WebviewWindow<R>> {
        let built = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("netspeed.html".into()))
            .additional_browser_args(crate::platform::WEBVIEW_ARGS)
            .title("Notcher speed")
            .inner_size(WIDTH, 40.0)
            .resizable(false)
            .decorations(false)
            .transparent(true)
            .shadow(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .focused(false)
            .visible(false)
            .build();
        match built {
            Ok(w) => {
                let _ = w.set_ignore_cursor_events(true);
                Some(w)
            }
            Err(err) => {
                log::warn!("could not create taskbar speed meter: {err}");
                None
            }
        }
    }

    fn hwnd_of<R: Runtime>(window: &tauri::WebviewWindow<R>) -> Option<HWND> {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        match window.window_handle().ok()?.as_raw() {
            RawWindowHandle::Win32(h) => Some(HWND(h.hwnd.get() as *mut _)),
            _ => None,
        }
    }

    fn target_rect() -> Option<(i32, i32, i32, i32)> {
        unsafe {
            let taskbar = FindWindowW(w!("Shell_TrayWnd"), None).ok()?;
            let mut r = RECT::default();
            GetWindowRect(taskbar, &mut r).ok()?;
            let tray_left = FindWindowExW(Some(taskbar), None, w!("TrayNotifyWnd"), None)
                .ok()
                .and_then(|tray| {
                    let mut t = RECT::default();
                    GetWindowRect(tray, &mut t).ok().map(|_| t.left)
                });
            let dpi = GetDpiForWindow(taskbar).max(96);
            let width = (WIDTH * dpi as f64 / 96.0).round() as i32;
            placement((r.left, r.top, r.right, r.bottom), tray_left, width)
        }
    }

    fn fullscreen_app() -> bool {
        unsafe { SHQueryUserNotificationState() }.is_ok_and(|s| {
            s == QUNS_BUSY || s == QUNS_RUNNING_D3D_FULL_SCREEN || s == QUNS_PRESENTATION_MODE
        })
    }

    /// Windows' "system" (taskbar) colour mode.
    fn light_taskbar() -> bool {
        let mut value: u32 = 0;
        let mut size = std::mem::size_of::<u32>() as u32;
        let status = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"),
                w!("SystemUsesLightTheme"),
                RRF_RT_REG_DWORD,
                None,
                Some(&mut value as *mut u32 as *mut _),
                Some(&mut size),
            )
        };
        status.is_ok() && value == 1
    }
}

#[cfg(test)]
mod tests {
    use super::placement;

    #[test]
    fn sits_left_of_the_tray_on_a_bottom_taskbar() {
        let bar = (0, 1032, 1920, 1080);
        assert_eq!(placement(bar, Some(1700), 78), Some((1618, 1032, 78, 48)));
    }

    #[test]
    fn falls_back_when_the_tray_is_unknown_and_skips_vertical_bars() {
        let bar = (0, 1032, 1920, 1080);
        let (x, ..) = placement(bar, None, 78).unwrap();
        assert!(x > 1400 && x < 1920 - 78);
        assert_eq!(placement((0, 0, 48, 1080), Some(10), 78), None);
    }
}
