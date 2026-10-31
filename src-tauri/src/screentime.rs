//! Screen time: how long the PC has been in use today, and in which apps.
//!
//! A background thread checks every few seconds whether someone is using the PC
//! (recent keyboard or mouse input, or a full-screen video or game) and which app is
//! in front. Only the app's name and a running total are kept, in `screentime.json`
//! in the app data folder. Nothing about what is on screen is recorded.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;
use tauri::{AppHandle, Runtime};

/// Emitted when the user becomes active or goes idle (used by the water reminder).
#[cfg_attr(not(windows), allow(dead_code))]
pub const ACTIVE_EVENT: &str = "screen://active";

/// How many apps the breakdown lists.
const TOP_APPS: usize = 5;

/// One day of screen time.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Day {
    /// Local date, `YYYY-MM-DD`.
    pub day: String,
    /// Seconds in use.
    pub total: u64,
    /// Seconds per app, by display name.
    pub apps: HashMap<String, u64>,
}

impl Day {
    /// Adds `secs` of use in `app` on `day`, starting a fresh day when the date changed.
    #[cfg_attr(not(windows), allow(dead_code))]
    pub fn add(&mut self, day: &str, app: Option<&str>, secs: u64) {
        if self.day != day {
            *self = Day {
                day: day.to_owned(),
                ..Day::default()
            };
        }
        self.total += secs;
        if let Some(app) = app.filter(|a| !a.is_empty()) {
            *self.apps.entry(app.to_owned()).or_default() += secs;
        }
    }

    /// Today's totals for the UI, with the most-used apps first.
    pub fn summary(&self) -> ScreenTime {
        let mut apps: Vec<AppTime> = self
            .apps
            .iter()
            .map(|(name, secs)| AppTime {
                name: name.clone(),
                secs: *secs,
            })
            .collect();
        apps.sort_by(|a, b| b.secs.cmp(&a.secs).then_with(|| a.name.cmp(&b.name)));
        apps.truncate(TOP_APPS);
        ScreenTime {
            total_secs: self.total,
            apps,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppTime {
    pub name: String,
    pub secs: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenTime {
    pub total_secs: u64,
    pub apps: Vec<AppTime>,
}

static TODAY: Mutex<Option<Day>> = Mutex::new(None);

#[tauri::command]
pub fn screen_time() -> ScreenTime {
    TODAY
        .lock()
        .ok()
        .and_then(|d| d.as_ref().map(Day::summary))
        .unwrap_or(ScreenTime {
            total_secs: 0,
            apps: Vec::new(),
        })
}

pub fn start<R: Runtime>(app: AppHandle<R>) {
    #[cfg(windows)]
    platform::spawn(app);
    #[cfg(not(windows))]
    let _ = app;
}

/// Turns an executable's name into something readable when Windows has no better name.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn friendly_exe(stem: &str) -> String {
    match stem.to_ascii_lowercase().as_str() {
        "msedge" => "Microsoft Edge".into(),
        "chrome" => "Google Chrome".into(),
        "firefox" => "Firefox".into(),
        "explorer" => "File Explorer".into(),
        "code" => "Visual Studio Code".into(),
        "windowsterminal" => "Terminal".into(),
        "winword" => "Word".into(),
        "excel" => "Excel".into(),
        "powerpnt" => "PowerPoint".into(),
        "olk" | "outlook" => "Outlook".into(),
        _ => {
            let mut chars = stem.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().chain(chars).collect(),
                None => String::new(),
            }
        }
    }
}

#[cfg(windows)]
mod platform {
    use super::{friendly_exe, Day, ACTIVE_EVENT, TODAY};
    use serde::Serialize;
    use std::path::{Path, PathBuf};
    use std::time::{Duration, Instant};
    use tauri::{AppHandle, Emitter, Manager, Runtime};
    use windows::core::{PCWSTR, PWSTR};
    use windows::Win32::{
        Foundation::{CloseHandle, HWND},
        Storage::FileSystem::{GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW},
        System::{
            SystemInformation::{GetLocalTime, GetTickCount},
            Threading::{
                OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
                PROCESS_QUERY_LIMITED_INFORMATION,
            },
        },
        UI::{
            Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO},
            Shell::{
                SHQueryUserNotificationState, QUNS_BUSY, QUNS_NOT_PRESENT, QUNS_PRESENTATION_MODE,
                QUNS_RUNNING_D3D_FULL_SCREEN,
            },
            WindowsAndMessaging::{GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId},
        },
    };

    const TICK: Duration = Duration::from_secs(5);
    /// No input for this long counts as away (unless a video or game is full screen).
    const IDLE_AFTER_MS: u32 = 3 * 60 * 1000;
    const SAVE_EVERY: Duration = Duration::from_secs(60);

    #[derive(Serialize, Clone, Copy)]
    struct Flag {
        value: bool,
    }

    pub fn spawn<R: Runtime>(app: AppHandle<R>) {
        let spawned = std::thread::Builder::new()
            .name("notcher-screentime".into())
            .spawn(move || run(app));
        if let Err(err) = spawned {
            log::error!("could not start screen time: {err}");
        }
    }

    fn run<R: Runtime>(app: AppHandle<R>) {
        let path = app
            .path()
            .app_data_dir()
            .ok()
            .map(|d| d.join("screentime.json"));
        let mut today: Day = path
            .as_ref()
            .and_then(|p| std::fs::read(p).ok())
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        // A file from an earlier day starts over.
        today.add(&local_date(), None, 0);
        publish(&today);
        let own_pid = std::process::id();
        let mut last = Instant::now();
        let mut saved_at = Instant::now();
        let mut was_active: Option<bool> = None;
        loop {
            std::thread::sleep(TICK);
            // Real time since the last check, capped so sleep or hibernation never counts.
            let secs = last.elapsed().min(TICK * 2).as_secs();
            last = Instant::now();
            let active = in_use();
            if was_active != Some(active) {
                was_active = Some(active);
                let _ = app.emit_to("island", ACTIVE_EVENT, Flag { value: active });
            }
            let day = local_date();
            let new_day = today.day != day;
            if active {
                let name = foreground_app(own_pid);
                today.add(&day, name.as_deref(), secs);
            } else if new_day {
                today.add(&day, None, 0);
            }
            publish(&today);
            if new_day || saved_at.elapsed() >= SAVE_EVERY {
                saved_at = Instant::now();
                save(path.as_deref(), &today);
            }
        }
    }

    fn publish(day: &Day) {
        if let Ok(mut t) = TODAY.lock() {
            *t = Some(day.clone());
        }
    }

    fn save(path: Option<&Path>, day: &Day) {
        if let (Some(path), Ok(json)) = (path, serde_json::to_vec(day)) {
            let _ = std::fs::write(path, json);
        }
    }

    fn local_date() -> String {
        let t = unsafe { GetLocalTime() };
        format!("{:04}-{:02}-{:02}", t.wYear, t.wMonth, t.wDay)
    }

    /// Someone is at the PC: recent input, or a full-screen video, game or slideshow.
    /// A locked PC or a screen saver never counts.
    fn in_use() -> bool {
        let state = unsafe { SHQueryUserNotificationState() }.ok();
        if state == Some(QUNS_NOT_PRESENT) {
            return false;
        }
        let mut info = LASTINPUTINFO {
            cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
            dwTime: 0,
        };
        if unsafe { GetLastInputInfo(&mut info) }.as_bool() {
            let idle = unsafe { GetTickCount() }.wrapping_sub(info.dwTime);
            if idle < IDLE_AFTER_MS {
                return true;
            }
        }
        matches!(
            state,
            Some(QUNS_BUSY | QUNS_RUNNING_D3D_FULL_SCREEN | QUNS_PRESENTATION_MODE)
        )
    }

    /// Display name of the app in front, or `None` for the desktop and Notcher itself.
    fn foreground_app(own_pid: u32) -> Option<String> {
        let hwnd = unsafe { GetForegroundWindow() };
        if hwnd.is_invalid() {
            return None;
        }
        let mut pid = 0u32;
        unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
        if pid == 0 || pid == own_pid {
            return None;
        }
        let exe = exe_path(pid)?;
        let stem = exe.file_stem()?.to_string_lossy().to_string();
        match stem.to_ascii_lowercase().as_str() {
            // Store apps run inside this host; their window title is the app's name.
            "applicationframehost" => window_title(hwnd),
            // The desktop, taskbar and lock screen are not apps.
            "lockapp" | "shellexperiencehost" | "startmenuexperiencehost" | "searchhost" => None,
            _ => Some(file_description(&exe).unwrap_or_else(|| friendly_exe(&stem))),
        }
    }

    fn exe_path(pid: u32) -> Option<PathBuf> {
        unsafe {
            let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
            let mut buf = [0u16; 1024];
            let mut len = buf.len() as u32;
            let ok = QueryFullProcessImageNameW(
                process,
                PROCESS_NAME_WIN32,
                PWSTR(buf.as_mut_ptr()),
                &mut len,
            )
            .is_ok();
            let _ = CloseHandle(process);
            ok.then(|| PathBuf::from(String::from_utf16_lossy(&buf[..len as usize])))
        }
    }

    fn window_title(hwnd: HWND) -> Option<String> {
        let mut buf = [0u16; 256];
        let len = unsafe { GetWindowTextW(hwnd, &mut buf) } as usize;
        let title = String::from_utf16_lossy(&buf[..len.min(buf.len())])
            .trim()
            .to_string();
        (!title.is_empty()).then_some(title)
    }

    /// The program's own name from its version info ("Google Chrome", "Spotify").
    fn file_description(exe: &Path) -> Option<String> {
        let wide: Vec<u16> = exe
            .as_os_str()
            .to_string_lossy()
            .encode_utf16()
            .chain(Some(0))
            .collect();
        let path = PCWSTR(wide.as_ptr());
        unsafe {
            let size = GetFileVersionInfoSizeW(path, None);
            if size == 0 {
                return None;
            }
            let mut data = vec![0u8; size as usize];
            GetFileVersionInfoW(path, None, size, data.as_mut_ptr().cast()).ok()?;
            // The first language and code page the file lists.
            let mut ptr = std::ptr::null_mut();
            let mut len = 0u32;
            let query: Vec<u16> = "\\VarFileInfo\\Translation\0".encode_utf16().collect();
            if !VerQueryValueW(
                data.as_ptr().cast(),
                PCWSTR(query.as_ptr()),
                &mut ptr,
                &mut len,
            )
            .as_bool()
                || len < 4
            {
                return None;
            }
            let pair = std::slice::from_raw_parts(ptr as *const u16, 2);
            let key: Vec<u16> = format!(
                "\\StringFileInfo\\{:04x}{:04x}\\FileDescription\0",
                pair[0], pair[1]
            )
            .encode_utf16()
            .collect();
            if !VerQueryValueW(
                data.as_ptr().cast(),
                PCWSTR(key.as_ptr()),
                &mut ptr,
                &mut len,
            )
            .as_bool()
                || len == 0
            {
                return None;
            }
            let text = std::slice::from_raw_parts(ptr as *const u16, len as usize);
            let end = text.iter().position(|&c| c == 0).unwrap_or(text.len());
            let name = String::from_utf16_lossy(&text[..end]).trim().to_string();
            (!name.is_empty() && name.len() <= 60).then_some(name)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adds_time_per_app_and_starts_over_each_day() {
        let mut d = Day::default();
        d.add("2026-10-31", Some("Google Chrome"), 60);
        d.add("2026-10-31", Some("Spotify"), 30);
        d.add("2026-10-31", Some("Google Chrome"), 15);
        d.add("2026-10-31", None, 5);
        assert_eq!(d.total, 110);
        let s = d.summary();
        assert_eq!(s.total_secs, 110);
        assert_eq!(s.apps[0].name, "Google Chrome");
        assert_eq!(s.apps[0].secs, 75);
        assert_eq!(s.apps[1].name, "Spotify");

        d.add("2026-11-01", Some("Word"), 10);
        assert_eq!(d.day, "2026-11-01");
        assert_eq!(d.total, 10);
        assert_eq!(d.summary().apps.len(), 1);
    }

    #[test]
    fn lists_only_the_top_apps() {
        let mut d = Day::default();
        for i in 0..8 {
            d.add("2026-10-31", Some(&format!("App {i}")), i + 1);
        }
        let s = d.summary();
        assert_eq!(s.apps.len(), TOP_APPS);
        assert_eq!(s.apps[0].name, "App 7");
    }

    #[test]
    fn exe_names_become_readable() {
        assert_eq!(friendly_exe("msedge"), "Microsoft Edge");
        assert_eq!(friendly_exe("spotify"), "Spotify");
        assert_eq!(friendly_exe(""), "");
    }
}
