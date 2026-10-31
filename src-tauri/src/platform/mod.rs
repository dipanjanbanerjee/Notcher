//! Operating-system specific behaviour for the island window.
//!
//! * placing the window (and, on Wayland, turning it into a layer-shell surface)
//! * making everything outside the island click-through
//! * noticing full-screen apps so the island can step aside
//! * following the cursor to another monitor

use crate::island::{self, HitRegion, IslandState, MonitorRect};
use crate::settings;
use serde::Serialize;
use std::{
    thread,
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, Runtime, WebviewWindow};

#[cfg(target_os = "linux")]
mod linux;
#[cfg(windows)]
mod windows;

#[cfg(target_os = "linux")]
use linux as backend;
#[cfg(target_os = "linux")]
pub use linux::before_gtk_init;
#[cfg(windows)]
use windows as backend;

/// Extra WebView2 switches for every Notcher window (Windows only).
///
/// Chromium pauses or slows pages it believes are hidden behind other windows. A small,
/// transparent, always-on-top window like the island can be misjudged as hidden,
/// which shows up as stalled or stuttering animations. These switches keep it
/// rendering at full rate. All windows must share the same switches because they
/// share one WebView2 profile; the first three are WebView2's own defaults.
#[cfg_attr(not(windows), allow(dead_code))]
pub const WEBVIEW_ARGS: &str = "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection,CalculateNativeWinOcclusion \
--disable-background-timer-throttling --disable-renderer-backgrounding --disable-backgrounding-occluded-windows";

/// Must run before Tauri initialises the windowing toolkit.
#[cfg(not(target_os = "linux"))]
pub fn before_gtk_init() {}

/// Emitted when a full-screen app appears or goes away on the island's monitor.
pub const SUPPRESS_EVENT: &str = "island://suppressed";
/// Emitted (where the OS cannot tell the web view itself) when the pointer enters or leaves the island.
#[cfg_attr(not(windows), allow(dead_code))]
pub const POINTER_EVENT: &str = "island://pointer";

#[derive(Debug, Clone, Copy, Default)]
pub struct PlatformMode {
    /// The window is a Wayland layer-shell surface; the compositor positions it and
    /// keeps it below full-screen windows, so no polling is needed.
    pub layer_shell: bool,
}

#[derive(Serialize, Clone, Copy)]
struct Flag {
    value: bool,
}

/// Prepares the freshly created island window for the current platform.
pub fn prepare_island<R: Runtime>(window: &WebviewWindow<R>) -> PlatformMode {
    backend::prepare_island(window)
}

/// Restricts mouse input to `region`; everything else passes through to the desktop.
pub fn apply_hit_region<R: Runtime>(window: &WebviewWindow<R>, region: HitRegion) {
    backend::apply_hit_region(window, region);
}

/// Moves the island to the top centre of the monitor under the cursor (or the first one).
pub fn place_on_best_monitor<R: Runtime>(window: &WebviewWindow<R>) -> Option<MonitorRect> {
    let monitors = backend::monitors(window);
    let monitor = backend::cursor_position(window)
        .and_then(|(x, y)| island::monitor_at(&monitors, x, y))
        .or_else(|| primary_monitor(window))
        .or_else(|| monitors.first().copied())?;
    move_to(window, &monitor);
    Some(monitor)
}

fn move_to<R: Runtime>(window: &WebviewWindow<R>, monitor: &MonitorRect) {
    let (x, y) = island::window_position_for(monitor);
    if let Err(err) = window.set_position(PhysicalPosition::new(x, y)) {
        log::warn!("could not move island: {err}");
    }
}

pub(crate) fn to_rect(m: &tauri::Monitor) -> MonitorRect {
    MonitorRect {
        x: m.position().x,
        y: m.position().y,
        width: m.size().width,
        height: m.size().height,
        scale: m.scale_factor(),
    }
}

fn primary_monitor<R: Runtime>(window: &WebviewWindow<R>) -> Option<MonitorRect> {
    window
        .primary_monitor()
        .ok()
        .flatten()
        .as_ref()
        .map(to_rect)
}

/// How often the cursor is checked for click-through (Windows only). About one frame
/// at 60 Hz, so hovering reacts as fast as the screen can show it.
const POINTER_POLL: Duration = Duration::from_millis(16);
/// How often monitors and full-screen apps are checked.
const SLOW_POLL: Duration = Duration::from_millis(400);

/// Background loop for the things the OS will not tell us about by event.
///
/// It talks to the OS directly (Win32 / its own X11 connection) instead of through
/// Tauri's window getters, which on Linux block the UI thread and can drop frames.
pub fn start_watcher<R: Runtime>(
    app: AppHandle<R>,
    mode: PlatformMode,
    initial: Option<MonitorRect>,
) {
    if mode.layer_shell {
        return;
    }
    let spawned = thread::Builder::new()
        .name("notcher-watcher".into())
        .spawn(move || {
            let Some(window) = app.get_webview_window("island") else {
                return;
            };
            let mut current_monitor = initial;
            let mut suppressed = false;
            let mut pointer = backend::PointerTracker::default();
            let mut last_slow = Instant::now() - SLOW_POLL;
            loop {
                if backend::POLLS_POINTER {
                    let region = app
                        .state::<IslandState>()
                        .hit_region
                        .lock()
                        .map(|r| *r)
                        .unwrap_or_default();
                    pointer.update(&window, region);
                }

                if last_slow.elapsed() >= SLOW_POLL {
                    last_slow = Instant::now();
                    let monitors = backend::monitors(&window);
                    if settings::follow_cursor(&app) {
                        let under_cursor = backend::cursor_position(&window)
                            .and_then(|(x, y)| island::monitor_at(&monitors, x, y));
                        if let Some(monitor) = under_cursor {
                            if current_monitor != Some(monitor) {
                                move_to(&window, &monitor);
                                current_monitor = Some(monitor);
                                pointer.invalidate();
                            }
                        }
                    }
                    // Keep the monitor fresh if resolution or layout changed.
                    if let Some(m) = current_monitor {
                        if !monitors.contains(&m) {
                            current_monitor =
                                island::monitor_at(&monitors, m.x as f64 + 1.0, m.y as f64 + 1.0)
                                    .or_else(|| monitors.first().copied());
                            if let Some(m) = current_monitor {
                                move_to(&window, &m);
                            }
                        }
                    }
                    let include_maximized = settings::flag(&app, "tuckOnMaximized", true);
                    let fullscreen = current_monitor.is_some_and(|m| {
                        backend::fullscreen_app_active(&window, &m, include_maximized)
                    });
                    if fullscreen != suppressed {
                        suppressed = fullscreen;
                        let _ = app.emit(SUPPRESS_EVENT, Flag { value: suppressed });
                    }
                }
                thread::sleep(if backend::POLLS_POINTER {
                    POINTER_POLL
                } else {
                    SLOW_POLL
                });
            }
        });
    if let Err(err) = spawned {
        log::error!("could not start window watcher: {err}");
    }
}

#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn emit_pointer<R: Runtime>(window: &WebviewWindow<R>, inside: bool) {
    let _ = window.emit(POINTER_EVENT, Flag { value: inside });
}
