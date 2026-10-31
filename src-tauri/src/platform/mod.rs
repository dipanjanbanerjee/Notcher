//! Operating-system specific behaviour for the island window.
//!
//! * placing the window (and, on Wayland, turning it into a layer-shell surface)
//! * making everything outside the island click-through
//! * noticing full-screen apps so the island can step aside
//! * following the cursor to another monitor

use crate::island::{self, HitRegion, IslandState, MonitorRect};
use crate::settings;
use serde::Serialize;
use std::{thread, time::Duration};
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

/// Moves the island to the top centre of the monitor under the cursor (or the primary one).
pub fn place_on_best_monitor<R: Runtime>(window: &WebviewWindow<R>) -> Option<MonitorRect> {
    let monitor = cursor_monitor(window).or_else(|| primary_monitor(window))?;
    let (x, y) = island::window_position_for(&monitor);
    if let Err(err) = window.set_position(PhysicalPosition::new(x, y)) {
        log::warn!("could not move island: {err}");
    }
    Some(monitor)
}

fn to_rect(m: &tauri::Monitor) -> MonitorRect {
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

fn cursor_monitor<R: Runtime>(window: &WebviewWindow<R>) -> Option<MonitorRect> {
    let cursor = window.cursor_position().ok()?;
    window
        .available_monitors()
        .ok()?
        .iter()
        .map(to_rect)
        .find(|m| m.contains(cursor.x, cursor.y))
}

/// Background loop for the things the OS will not tell us about by event.
pub fn start_watcher<R: Runtime>(app: AppHandle<R>, mode: PlatformMode) {
    if mode.layer_shell {
        return;
    }
    let spawned = thread::Builder::new()
        .name("notcher-watcher".into())
        .spawn(move || {
            let Some(window) = app.get_webview_window("island") else {
                return;
            };
            let mut current_monitor = None;
            let mut suppressed = false;
            let mut pointer = backend::PointerTracker::default();
            let mut tick: u32 = 0;
            loop {
                let region = app
                    .state::<IslandState>()
                    .hit_region
                    .lock()
                    .map(|r| *r)
                    .unwrap_or_default();
                pointer.update(&window, region);

                // Slower checks, roughly twice a second.
                if tick % 15 == 0 {
                    if settings::follow_cursor(&app) {
                        if let Some(monitor) = cursor_monitor(&window) {
                            if current_monitor != Some(monitor) {
                                let (x, y) = island::window_position_for(&monitor);
                                let _ = window.set_position(PhysicalPosition::new(x, y));
                                current_monitor = Some(monitor);
                                pointer.invalidate();
                            }
                        }
                    }
                    let fullscreen = backend::fullscreen_app_active(&window);
                    if fullscreen != suppressed {
                        suppressed = fullscreen;
                        let _ = app.emit(SUPPRESS_EVENT, Flag { value: suppressed });
                    }
                }
                tick = tick.wrapping_add(1);
                thread::sleep(Duration::from_millis(33));
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
