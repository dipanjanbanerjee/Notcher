//! A small internet speed meter on the Windows taskbar, next to the tray icons.
//!
//! It is its own tiny transparent window that sits over the taskbar, ignores the
//! mouse, and is re-raised every second (clicking the taskbar would otherwise cover
//! it). Speeds come from the vitals sampler, which already measures them.
//!
//! It can also show how much data was used today, kept across restarts in
//! `usage.json`, and its text size and side of the taskbar can be changed in Settings.

use serde::{Deserialize, Serialize};

#[cfg_attr(not(windows), allow(dead_code))]
pub const UPDATE_EVENT: &str = "netspeed://update";
#[cfg_attr(not(windows), allow(dead_code))]
pub const LABEL: &str = "netspeed";

#[cfg_attr(not(windows), allow(dead_code))]
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Speed {
    /// Bytes per second.
    pub rx: u64,
    pub tx: u64,
    /// The taskbar is light, so the text should be dark.
    pub light_taskbar: bool,
    /// Bytes sent and received today, when that is shown.
    pub today: Option<u64>,
    /// "small", "medium" or "large".
    pub size: String,
    /// The meter sits on the left of the taskbar, so its text aligns left.
    pub left: bool,
}

/// Text size of the meter.
#[cfg_attr(not(windows), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Size {
    Small,
    Medium,
    Large,
}

#[cfg_attr(not(windows), allow(dead_code))]
impl Size {
    pub fn parse(s: &str) -> Size {
        match s {
            "small" => Size::Small,
            "large" => Size::Large,
            _ => Size::Medium,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Size::Small => "small",
            Size::Medium => "medium",
            Size::Large => "large",
        }
    }

    /// Logical width of the meter for this size, wider when today's usage is shown.
    pub fn width(self, today: bool) -> f64 {
        let (speeds, usage) = match self {
            Size::Small => (80.0, 70.0),
            Size::Medium => (94.0, 80.0),
            Size::Large => (108.0, 92.0),
        };
        if today {
            speeds + usage
        } else {
            speeds
        }
    }
}

/// Where the meter goes, in physical pixels. On the right it sits just left of the
/// tray; on the left, just right of `left_anchor` (the Start button on Windows 10) or
/// at the taskbar's left edge. `None` for a vertical or hidden taskbar.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn placement(
    taskbar: (i32, i32, i32, i32),
    tray_left: Option<i32>,
    left_anchor: Option<i32>,
    width: i32,
    on_left: bool,
) -> Option<(i32, i32, i32, i32)> {
    let (left, top, right, bottom) = taskbar;
    let (w, h) = (right - left, bottom - top);
    if w <= 0 || h <= 0 || h > w {
        return None;
    }
    if on_left {
        let x = left_anchor
            .filter(|x| *x >= left && *x + width < right)
            .unwrap_or(left)
            + 8;
        return Some((x, top, width, h));
    }
    let anchor = tray_left
        .filter(|x| *x > left + width && *x <= right)
        .unwrap_or(right - (w / 6).max(width * 3));
    Some((anchor - width - 4, top, width, h))
}

/// Moves the meter sideways by the user's fine-tuning (physical pixels, positive is
/// right), keeping it on the taskbar.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn nudged(x: i32, width: i32, offset: i32, taskbar_left: i32, taskbar_right: i32) -> i32 {
    let max = (taskbar_right - width).max(taskbar_left);
    (x + offset).clamp(taskbar_left, max)
}

/// Data used on one day, saved so restarts don't reset it.
#[cfg_attr(not(windows), allow(dead_code))]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DailyUsage {
    /// Local date, "YYYY-MM-DD".
    pub day: String,
    pub bytes: u64,
}

#[cfg_attr(not(windows), allow(dead_code))]
impl DailyUsage {
    /// Adds bytes used on `today`, starting from zero when the date has changed.
    pub fn add(&mut self, today: &str, bytes: u64) {
        if self.day != today {
            self.day = today.to_owned();
            self.bytes = 0;
        }
        self.bytes = self.bytes.saturating_add(bytes);
    }
}

pub fn start<R: tauri::Runtime>(app: tauri::AppHandle<R>) {
    #[cfg(windows)]
    platform::spawn(app);
    #[cfg(not(windows))]
    let _ = app;
}

#[cfg(windows)]
mod platform {
    use super::{nudged, placement, DailyUsage, Size, Speed, LABEL, UPDATE_EVENT};
    use crate::settings;
    use std::path::PathBuf;
    use std::time::{Duration, Instant};
    use tauri::{AppHandle, Emitter, Manager, Runtime, WebviewUrl, WebviewWindowBuilder};
    use windows::core::w;
    use windows::Win32::{
        Foundation::{HWND, RECT},
        System::{
            Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD},
            SystemInformation::GetLocalTime,
        },
        UI::{
            HiDpi::GetDpiForWindow,
            Shell::{
                SHQueryUserNotificationState, QUNS_BUSY, QUNS_PRESENTATION_MODE,
                QUNS_RUNNING_D3D_FULL_SCREEN,
            },
            WindowsAndMessaging::{
                FindWindowExW, FindWindowW, GetWindowRect, IsWindowVisible, SetWindowPos,
                HWND_TOPMOST, SWP_NOACTIVATE, SWP_SHOWWINDOW,
            },
        },
    };

    /// How often today's usage is written to disk.
    const SAVE_EVERY: Duration = Duration::from_secs(30);

    /// Counts today's data use, whether or not the meter is showing.
    struct Usage {
        path: Option<PathBuf>,
        today: DailyUsage,
        last_total: u64,
        saved_at: Instant,
    }

    impl Usage {
        fn load<R: Runtime>(app: &AppHandle<R>) -> Usage {
            let path = app.path().app_data_dir().ok().map(|d| d.join("usage.json"));
            let today = path
                .as_ref()
                .and_then(|p| std::fs::read(p).ok())
                .and_then(|b| serde_json::from_slice(&b).ok())
                .unwrap_or_default();
            Usage {
                path,
                today,
                last_total: crate::vitals::net_total(),
                saved_at: Instant::now(),
            }
        }

        fn update(&mut self) -> u64 {
            let total = crate::vitals::net_total();
            let delta = total.saturating_sub(self.last_total);
            self.last_total = total;
            let day = local_date();
            let new_day = self.today.day != day;
            self.today.add(&day, delta);
            if new_day || self.saved_at.elapsed() >= SAVE_EVERY {
                self.save();
            }
            self.today.bytes
        }

        fn save(&mut self) {
            self.saved_at = Instant::now();
            if let (Some(path), Ok(json)) = (&self.path, serde_json::to_vec(&self.today)) {
                let _ = std::fs::write(path, json);
            }
        }
    }

    fn local_date() -> String {
        let t = unsafe { GetLocalTime() };
        format!("{:04}-{:02}-{:02}", t.wYear, t.wMonth, t.wDay)
    }

    pub fn spawn<R: Runtime>(app: AppHandle<R>) {
        let spawned = std::thread::Builder::new()
            .name("notcher-netspeed".into())
            .spawn(move || {
                let mut usage = Usage::load(&app);
                loop {
                    let today = usage.update();
                    tick(&app, today);
                    std::thread::sleep(Duration::from_secs(1));
                }
            });
        if let Err(err) = spawned {
            log::error!("could not start taskbar speed meter: {err}");
        }
    }

    fn tick<R: Runtime>(app: &AppHandle<R>, today: u64) {
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
        let size = Size::parse(&settings::text(app, "taskbarSpeedSize", "medium"));
        let show_today = settings::flag(app, "taskbarSpeedToday", true);
        let on_left = settings::text(app, "taskbarSpeedPosition", "right") == "left";
        let offset = settings::number(app, "taskbarSpeedOffset", 0.0);
        match target_rect(size.width(show_today), on_left, offset) {
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
                today: show_today.then_some(today),
                size: size.name().to_owned(),
                left: on_left,
            };
            let _ = app.emit_to(LABEL, UPDATE_EVENT, speed);
        }
    }

    fn create<R: Runtime>(app: &AppHandle<R>) -> Option<tauri::WebviewWindow<R>> {
        let built = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("netspeed.html".into()))
            .additional_browser_args(crate::platform::WEBVIEW_ARGS)
            .title("Notcher speed")
            .inner_size(Size::Medium.width(true), 40.0)
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

    fn target_rect(
        logical_width: f64,
        on_left: bool,
        logical_offset: f64,
    ) -> Option<(i32, i32, i32, i32)> {
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
            // Windows 10 has a real Start button window; Windows 11 draws it inside the
            // taskbar, where its icons are centred and the left end is free.
            let start_right = FindWindowExW(Some(taskbar), None, w!("Start"), None)
                .ok()
                .filter(|s| IsWindowVisible(*s).as_bool())
                .and_then(|start| {
                    let mut t = RECT::default();
                    GetWindowRect(start, &mut t).ok().map(|_| t.right)
                });
            let dpi = GetDpiForWindow(taskbar).max(96);
            let scale = dpi as f64 / 96.0;
            let width = (logical_width * scale).round() as i32;
            let offset = (logical_offset * scale).round() as i32;
            placement(
                (r.left, r.top, r.right, r.bottom),
                tray_left,
                start_right,
                width,
                on_left,
            )
            .map(|(x, y, w, h)| (nudged(x, w, offset, r.left, r.right), y, w, h))
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
    use super::{nudged, placement, DailyUsage, Size};

    #[test]
    fn sits_left_of_the_tray_on_a_bottom_taskbar() {
        let bar = (0, 1032, 1920, 1080);
        assert_eq!(
            placement(bar, Some(1700), None, 78, false),
            Some((1618, 1032, 78, 48))
        );
    }

    #[test]
    fn falls_back_when_the_tray_is_unknown_and_skips_vertical_bars() {
        let bar = (0, 1032, 1920, 1080);
        let (x, ..) = placement(bar, None, None, 78, false).unwrap();
        assert!(x > 1400 && x < 1920 - 78);
        assert_eq!(placement((0, 0, 48, 1080), Some(10), None, 78, false), None);
    }

    #[test]
    fn left_side_goes_after_the_start_button_or_the_edge() {
        let bar = (0, 1032, 1920, 1080);
        assert_eq!(
            placement(bar, Some(1700), Some(48), 150, true),
            Some((56, 1032, 150, 48))
        );
        assert_eq!(
            placement(bar, Some(1700), None, 150, true),
            Some((8, 1032, 150, 48))
        );
    }

    #[test]
    fn usage_restarts_each_day() {
        let mut u = DailyUsage::default();
        u.add("2026-10-31", 100);
        u.add("2026-10-31", 50);
        assert_eq!(u.bytes, 150);
        u.add("2026-11-01", 7);
        assert_eq!((u.day.as_str(), u.bytes), ("2026-11-01", 7));
    }

    #[test]
    fn bigger_text_and_usage_make_the_meter_wider() {
        assert!(Size::Large.width(false) > Size::Small.width(false));
        assert!(Size::Medium.width(true) > Size::Medium.width(false));
        assert_eq!(Size::parse("nonsense"), Size::Medium);
    }

    #[test]
    fn nudging_moves_the_meter_but_keeps_it_on_the_taskbar() {
        assert_eq!(nudged(1500, 90, -40, 0, 1920), 1460);
        assert_eq!(nudged(1500, 90, 25, 0, 1920), 1525);
        assert_eq!(nudged(1800, 90, 500, 0, 1920), 1830);
        assert_eq!(nudged(20, 90, -500, 0, 1920), 0);
    }
}
