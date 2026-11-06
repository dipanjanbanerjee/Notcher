//! A small internet speed meter on the Windows taskbar, next to the tray icons.
//!
//! It is its own tiny transparent window that sits over the taskbar and ignores the
//! mouse. It is *owned* by the taskbar window, so Windows itself keeps it directly
//! above the taskbar in the z-order: clicking the taskbar raises both together (no
//! flicker), and when Windows lowers the taskbar behind a full-screen app the meter
//! goes down with it. Earlier versions were a separate always-on-top window that had to
//! be pushed back over the taskbar after every click, which showed as blinking.
//!
//! What it watches comes from Windows events (foreground changes, windows moving,
//! showing, hiding, being cloaked), not from polling: the taskbar sliding away
//! (auto-hide), a full-screen window in front of it, or the taskbar moving. Each show
//! or hide is written to the log with its reason (`taskbar meter:` lines). Speeds come
//! from the vitals sampler, which already measures them, once a second.
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

/// At least half of the taskbar's thickness is on its monitor. An auto-hidden taskbar
/// leaves only a sliver of about 2 pixels on screen. Rectangles are (left, top, right,
/// bottom) in physical pixels.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn mostly_on_screen(bar: (i32, i32, i32, i32), monitor: (i32, i32, i32, i32)) -> bool {
    let visible_w = bar.2.min(monitor.2) - bar.0.max(monitor.0);
    let visible_h = bar.3.min(monitor.3) - bar.1.max(monitor.1);
    let (w, h) = (bar.2 - bar.0, bar.3 - bar.1);
    if visible_w <= 0 || visible_h <= 0 {
        return false;
    }
    if w >= h {
        visible_h * 2 >= h
    } else {
        visible_w * 2 >= w
    }
}

/// Shows or hides the meter only once the new answer has held for a short moment, so
/// a window flashing past (a menu, a flyout opening) can't make it blink.
#[cfg_attr(not(windows), allow(dead_code))]
#[derive(Debug, Default)]
pub struct Settle {
    shown: Option<bool>,
    /// The answer that differs from `shown`, and since when it has been seen.
    pending: Option<(bool, std::time::Instant)>,
}

#[cfg_attr(not(windows), allow(dead_code))]
impl Settle {
    /// How long a new answer must hold before it is applied.
    pub const DELAY: std::time::Duration = std::time::Duration::from_millis(150);

    /// Feeds the latest reading taken at `now`; returns the state to apply when it
    /// changes.
    pub fn update(&mut self, visible: bool, now: std::time::Instant) -> Option<bool> {
        if self.shown.is_none() {
            self.shown = Some(visible);
            return Some(visible);
        }
        if self.shown == Some(visible) {
            self.pending = None;
            return None;
        }
        match self.pending {
            Some((v, since)) if v == visible => {
                if now.duration_since(since) >= Self::DELAY {
                    self.pending = None;
                    self.shown = Some(visible);
                    Some(visible)
                } else {
                    None
                }
            }
            _ => {
                self.pending = Some((visible, now));
                None
            }
        }
    }

    /// A change is waiting to be confirmed, so the caller should look again soon.
    pub fn waiting(&self) -> bool {
        self.pending.is_some()
    }
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
    use super::{nudged, placement, DailyUsage, Settle, Size, Speed, LABEL, UPDATE_EVENT};
    use crate::settings;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicIsize, Ordering};
    use std::sync::mpsc::{self, RecvTimeoutError, Sender};
    use std::sync::{Mutex, OnceLock};
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
            Accessibility::{SetWinEventHook, HWINEVENTHOOK},
            HiDpi::GetDpiForWindow,
            WindowsAndMessaging::{
                DispatchMessageW, FindWindowExW, FindWindowW, GetAncestor, GetForegroundWindow,
                GetMessageW, GetWindow, GetWindowLongPtrW, GetWindowLongW, GetWindowRect, IsWindow,
                IsWindowVisible, SetWindowLongPtrW, SetWindowPos, TranslateMessage,
                EVENT_OBJECT_CLOAKED, EVENT_OBJECT_HIDE, EVENT_OBJECT_LOCATIONCHANGE,
                EVENT_OBJECT_SHOW, EVENT_OBJECT_UNCLOAKED, EVENT_SYSTEM_FOREGROUND,
                EVENT_SYSTEM_MINIMIZEEND, EVENT_SYSTEM_MINIMIZESTART, GA_ROOT, GWLP_HWNDPARENT,
                GWL_EXSTYLE, GW_HWNDPREV, HWND_TOP, HWND_TOPMOST, MSG, OBJID_WINDOW,
                SWP_HIDEWINDOW, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER,
                SWP_SHOWWINDOW, WINEVENT_OUTOFCONTEXT, WINEVENT_SKIPOWNPROCESS, WS_EX_TOPMOST,
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

    /// How often the speeds are sent to the meter. The meter also re-checks the
    /// taskbar then, as a safety net for an event Windows didn't send.
    const SPEED_EVERY: Duration = Duration::from_secs(1);
    /// Events come in bursts (a window animating sends dozens); wait this long after
    /// the first one and handle the whole burst at once.
    const COALESCE: Duration = Duration::from_millis(30);

    /// Woken by the window-event hooks.
    static WAKE: OnceLock<Mutex<Sender<()>>> = OnceLock::new();
    /// The taskbar and the window in front, so the hook can ignore everything else.
    static TASKBAR: AtomicIsize = AtomicIsize::new(0);
    static FOREGROUND: AtomicIsize = AtomicIsize::new(0);

    fn wake() {
        if let Some(tx) = WAKE.get().and_then(|m| m.lock().ok()) {
            let _ = tx.send(());
        }
    }

    pub fn spawn<R: Runtime>(app: AppHandle<R>) {
        let (tx, rx) = mpsc::channel();
        let _ = WAKE.set(Mutex::new(tx));
        start_event_hooks();
        let spawned = std::thread::Builder::new()
            .name("notcher-netspeed".into())
            .spawn(move || {
                let mut usage = Usage::load(&app);
                let mut meter = Meter::default();
                let mut last_speed = Instant::now() - SPEED_EVERY;
                loop {
                    let speed_due = last_speed.elapsed() >= SPEED_EVERY;
                    let today = if speed_due {
                        last_speed = Instant::now();
                        Some(usage.update())
                    } else {
                        None
                    };
                    meter.tick(&app, today);
                    let mut wait = SPEED_EVERY.saturating_sub(last_speed.elapsed());
                    if meter.settle.waiting() {
                        wait = wait.min(Settle::DELAY / 2);
                    }
                    match rx.recv_timeout(wait) {
                        Ok(()) => {
                            std::thread::sleep(COALESCE);
                            while rx.try_recv().is_ok() {}
                        }
                        Err(RecvTimeoutError::Timeout) => {}
                        Err(RecvTimeoutError::Disconnected) => std::thread::sleep(wait),
                    }
                }
            });
        if let Err(err) = spawned {
            log::error!("could not start taskbar speed meter: {err}");
        }
    }

    /// Listens for the window events that can change whether the taskbar is visible
    /// or where it is, and wakes the meter for the ones that matter.
    fn start_event_hooks() {
        unsafe extern "system" fn on_event(
            _hook: HWINEVENTHOOK,
            event: u32,
            hwnd: HWND,
            object: i32,
            _child: i32,
            _thread: u32,
            _time: u32,
        ) {
            if event == EVENT_SYSTEM_FOREGROUND {
                FOREGROUND.store(hwnd.0 as isize, Ordering::Relaxed);
                wake();
                return;
            }
            // Only whole windows: the cursor and controls inside windows send these too.
            if object != OBJID_WINDOW.0 || hwnd.is_invalid() {
                return;
            }
            let raw = hwnd.0 as isize;
            let taskbar = TASKBAR.load(Ordering::Relaxed);
            if event == EVENT_OBJECT_LOCATIONCHANGE {
                // The taskbar sliding (auto-hide) or the app in front resizing (going
                // full-screen, or back).
                if raw == taskbar || raw == FOREGROUND.load(Ordering::Relaxed) {
                    wake();
                }
                return;
            }
            // Shown, hidden, cloaked, minimised: top-level windows only.
            if raw == taskbar || GetAncestor(hwnd, GA_ROOT) == hwnd {
                wake();
            }
        }
        let spawned = std::thread::Builder::new()
            .name("notcher-netspeed-hook".into())
            .spawn(|| unsafe {
                let ranges = [
                    (EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_FOREGROUND),
                    (EVENT_SYSTEM_MINIMIZESTART, EVENT_SYSTEM_MINIMIZEEND),
                    (EVENT_OBJECT_SHOW, EVENT_OBJECT_HIDE),
                    (EVENT_OBJECT_LOCATIONCHANGE, EVENT_OBJECT_LOCATIONCHANGE),
                    (EVENT_OBJECT_CLOAKED, EVENT_OBJECT_UNCLOAKED),
                ];
                let mut hooked = 0;
                for (min, max) in ranges {
                    let hook = SetWinEventHook(
                        min,
                        max,
                        None,
                        Some(on_event),
                        0,
                        0,
                        WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
                    );
                    if hook.is_invalid() {
                        log::warn!(
                            "taskbar meter: could not watch window events {min:#x}-{max:#x}"
                        );
                    } else {
                        hooked += 1;
                    }
                }
                if hooked == 0 {
                    log::warn!("taskbar meter: no window events; it re-checks once a second");
                    return;
                }
                FOREGROUND.store(GetForegroundWindow().0 as isize, Ordering::Relaxed);
                // Out-of-context hooks are delivered through this thread's messages.
                let mut msg = MSG::default();
                while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            });
        if let Err(err) = spawned {
            log::warn!("taskbar meter: could not start the event hooks: {err}");
        }
    }

    #[derive(Default)]
    struct Meter {
        settle: Settle,
        /// Where the meter was last put, to skip moving it when nothing changed.
        placed: Option<(i32, i32, i32, i32)>,
        visible: bool,
        /// The taskbar the meter is attached to (owned by).
        owner: isize,
        /// Last reason given for hiding, so the log isn't repeated.
        reason: String,
    }

    impl Meter {
        /// Keeps the meter on the taskbar and hides it while the taskbar can't be seen.
        /// `today` is set once a second, when the speeds are sent too.
        fn tick<R: Runtime>(&mut self, app: &AppHandle<R>, today: Option<u64>) {
            let existing = app.get_webview_window(LABEL);
            if !settings::flag(app, "taskbarSpeed", true) {
                if let Some(w) = existing {
                    let _ = w.destroy();
                    log::info!("taskbar meter: turned off in Settings");
                }
                *self = Meter::default();
                return;
            }
            let window = match existing {
                Some(w) => w,
                None => match create(app) {
                    Some(w) => {
                        *self = Meter::default();
                        w
                    }
                    None => return,
                },
            };
            let Some(hwnd) = hwnd_of(&window) else { return };
            let size = Size::parse(&settings::text(app, "taskbarSpeedSize", "medium"));
            let show_today = settings::flag(app, "taskbarSpeedToday", true);
            let on_left = settings::text(app, "taskbarSpeedPosition", "right") == "left";
            let offset = settings::number(app, "taskbarSpeedOffset", 0.0);
            let taskbar = unsafe { FindWindowW(w!("Shell_TrayWnd"), None) }.ok();
            TASKBAR.store(
                taskbar.map(|t| t.0 as isize).unwrap_or_default(),
                Ordering::Relaxed,
            );
            if let Some(t) = taskbar {
                self.attach(hwnd, t);
            }
            let rect =
                taskbar.and_then(|t| target_rect(t, size.width(show_today), on_left, offset));
            let hidden_because = match (taskbar, rect) {
                (None, _) => Some("no taskbar found (Explorer is restarting?)".to_owned()),
                (Some(_), None) => Some("the taskbar is vertical or has no size".to_owned()),
                (Some(t), Some(_)) => crate::platform::taskbar_hidden_reason(t),
            };
            if let Some(change) = self.settle.update(hidden_because.is_none(), Instant::now()) {
                self.visible = change;
                if change {
                    log::info!("taskbar meter: show (the taskbar is visible)");
                } else {
                    let why = hidden_because.clone().unwrap_or_default();
                    log::info!("taskbar meter: hide ({why})");
                    self.reason = why;
                    self.placed = None;
                    unsafe {
                        let _ = SetWindowPos(
                            hwnd,
                            None,
                            0,
                            0,
                            0,
                            0,
                            SWP_HIDEWINDOW
                                | SWP_NOMOVE
                                | SWP_NOSIZE
                                | SWP_NOZORDER
                                | SWP_NOACTIVATE,
                        );
                    }
                }
            } else if !self.visible {
                // Still hidden, but perhaps for another reason now.
                if let Some(why) = hidden_because.filter(|w| *w != self.reason) {
                    log::info!("taskbar meter: stays hidden ({why})");
                    self.reason = why;
                }
            }
            if !self.visible {
                return;
            }
            if let (Some(taskbar), Some(place)) = (taskbar, rect) {
                let covered = crate::platform::is_above(taskbar, hwnd);
                let moved = self.placed != Some(place) || actual_rect(hwnd) != Some(place);
                let hidden = !unsafe { IsWindowVisible(hwnd) }.as_bool();
                if covered || moved || hidden {
                    if self.placed != Some(place) {
                        log::info!(
                            "taskbar meter: placed at ({}, {}) {}x{} px",
                            place.0,
                            place.1,
                            place.2,
                            place.3
                        );
                    } else if covered {
                        log::info!("taskbar meter: back above the taskbar");
                    }
                    self.placed = Some(place);
                    let (x, y, w, h) = place;
                    unsafe {
                        let _ = SetWindowPos(
                            hwnd,
                            Some(slot_above(taskbar, hwnd)),
                            x,
                            y,
                            w,
                            h,
                            SWP_NOACTIVATE | SWP_SHOWWINDOW,
                        );
                    }
                }
            }
            let Some(today) = today else { return };
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

        /// Makes the taskbar the meter's owner, so Windows keeps the meter right above
        /// it. Done again when Explorer restarts and the taskbar is a new window.
        fn attach(&mut self, meter: HWND, taskbar: HWND) {
            let raw = taskbar.0 as isize;
            let current = unsafe { GetWindowLongPtrW(meter, GWLP_HWNDPARENT) };
            if self.owner == raw && current == raw {
                return;
            }
            if self.owner != 0 && !unsafe { IsWindow(Some(HWND(self.owner as *mut _))) }.as_bool() {
                log::info!("taskbar meter: the taskbar was re-created (Explorer restarted)");
            }
            unsafe { SetWindowLongPtrW(meter, GWLP_HWNDPARENT, raw) };
            self.owner = raw;
            self.placed = None;
            log::info!("taskbar meter: attached to the taskbar");
        }
    }

    /// Where in the z-order to put the meter: just above the taskbar, in the same band
    /// (always-on-top or not) as the taskbar is in right now.
    fn slot_above(taskbar: HWND, meter: HWND) -> HWND {
        let topmost = unsafe { GetWindowLongW(taskbar, GWL_EXSTYLE) } as u32 & WS_EX_TOPMOST.0 != 0;
        let above = unsafe { GetWindow(taskbar, GW_HWNDPREV) }.unwrap_or_default();
        if above.is_invalid() {
            if topmost {
                HWND_TOPMOST
            } else {
                HWND_TOP
            }
        } else if above == meter {
            // Already right there: keep the slot, only move or resize.
            unsafe { GetWindow(meter, GW_HWNDPREV) }
                .ok()
                .filter(|h| !h.is_invalid())
                .unwrap_or(if topmost { HWND_TOPMOST } else { HWND_TOP })
        } else {
            above
        }
    }

    fn actual_rect(hwnd: HWND) -> Option<(i32, i32, i32, i32)> {
        let mut r = RECT::default();
        unsafe { GetWindowRect(hwnd, &mut r) }.ok()?;
        Some((r.left, r.top, r.right - r.left, r.bottom - r.top))
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
            .skip_taskbar(true)
            .focused(false)
            .visible(false)
            .build();
        match built {
            Ok(w) => {
                let _ = w.set_ignore_cursor_events(true);
                log::info!("taskbar meter: created");
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
        taskbar: HWND,
        logical_width: f64,
        on_left: bool,
        logical_offset: f64,
    ) -> Option<(i32, i32, i32, i32)> {
        unsafe {
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
            // The taskbar's own DPI, so the meter is the right size on a scaled or
            // second monitor.
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
    use super::{mostly_on_screen, nudged, placement, DailyUsage, Settle, Size};
    use std::time::{Duration, Instant};

    #[test]
    fn an_auto_hidden_taskbar_counts_as_off_screen() {
        let monitor = (0, 0, 1920, 1080);
        assert!(mostly_on_screen((0, 1032, 1920, 1080), monitor));
        // Slid down, leaving 2 pixels showing.
        assert!(!mostly_on_screen((0, 1078, 1920, 1126), monitor));
        // Half way through sliding back.
        assert!(mostly_on_screen((0, 1056, 1920, 1104), monitor));
        // A taskbar on the left edge, hidden and shown.
        assert!(!mostly_on_screen((-46, 0, 2, 1080), monitor));
        assert!(mostly_on_screen((0, 0, 48, 1080), monitor));
        // On another monitor entirely.
        assert!(!mostly_on_screen((1920, 1032, 3840, 1080), monitor));
    }

    #[test]
    fn the_meter_only_changes_once_the_new_answer_holds() {
        let t0 = Instant::now();
        let at = |ms: u64| t0 + Duration::from_millis(ms);
        let mut s = Settle::default();
        assert_eq!(s.update(true, at(0)), Some(true));
        // A flyout flashing past for a moment does nothing.
        assert_eq!(s.update(false, at(10)), None);
        assert!(s.waiting());
        assert_eq!(s.update(true, at(60)), None);
        assert!(!s.waiting());
        // A full-screen video that stays does.
        assert_eq!(s.update(false, at(100)), None);
        assert_eq!(s.update(false, at(200)), None);
        assert_eq!(s.update(false, at(260)), Some(false));
        assert_eq!(s.update(false, at(300)), None);
        // And the meter comes back when the taskbar does.
        assert_eq!(s.update(true, at(400)), None);
        assert_eq!(s.update(true, at(560)), Some(true));
    }

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
