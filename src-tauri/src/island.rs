//! Island window geometry: where the window sits and which part of it accepts input.
//!
//! The island window is a fixed-size transparent surface. The visible island shape
//! is drawn and animated inside it by the web UI, which reports the shape's bounds
//! back as a [`HitRegion`]. Everything outside that region must let clicks pass
//! through to the apps underneath.

use serde::{Deserialize, Serialize};
use std::sync::Mutex;

/// Logical size of the island window. Large enough for the fully open panel plus shadow.
pub const WINDOW_WIDTH: f64 = 760.0;
pub const WINDOW_HEIGHT: f64 = 400.0;

/// Rectangle in logical (CSS) pixels, relative to the island window's top-left corner.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct HitRegion {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl HitRegion {
    /// Keep the region inside the window and make sure it has a sane, non-negative size.
    pub fn clamped(self) -> Self {
        let finite = |v: f64| if v.is_finite() { v } else { 0.0 };
        let x = finite(self.x).clamp(0.0, WINDOW_WIDTH);
        let y = finite(self.y).clamp(0.0, WINDOW_HEIGHT);
        let width = finite(self.width).max(0.0).min(WINDOW_WIDTH - x);
        let height = finite(self.height).max(0.0).min(WINDOW_HEIGHT - y);
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Whether a point in logical window coordinates falls inside the region.
    #[cfg_attr(not(windows), allow(dead_code))]
    pub fn contains(&self, px: f64, py: f64) -> bool {
        px >= self.x && px < self.x + self.width && py >= self.y && py < self.y + self.height
    }
}

/// Shared, thread-safe island state.
#[derive(Default)]
pub struct IslandState {
    pub hit_region: Mutex<HitRegion>,
}

/// A monitor's bounds in physical pixels plus its scale factor.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MonitorRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub scale: f64,
}

impl MonitorRect {
    pub fn contains(&self, px: f64, py: f64) -> bool {
        px >= self.x as f64
            && px < self.x as f64 + self.width as f64
            && py >= self.y as f64
            && py < self.y as f64 + self.height as f64
    }
}

/// The monitor containing a point in physical screen pixels.
pub fn monitor_at(monitors: &[MonitorRect], x: f64, y: f64) -> Option<MonitorRect> {
    monitors.iter().copied().find(|m| m.contains(x, y))
}

/// Physical position for the island window so it is centred at the top of `monitor`.
pub fn window_position_for(monitor: &MonitorRect) -> (i32, i32) {
    let window_w = (WINDOW_WIDTH * monitor.scale).round() as i32;
    let x = monitor.x + (monitor.width as i32 - window_w) / 2;
    (x, monitor.y)
}

/// Converts a point in physical screen pixels into logical island-window coordinates.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn to_window_logical(point: (f64, f64), window_origin: (i32, i32), scale: f64) -> (f64, f64) {
    let scale = if scale > 0.0 { scale } else { 1.0 };
    (
        (point.0 - window_origin.0 as f64) / scale,
        (point.1 - window_origin.1 as f64) / scale,
    )
}

/// Whether the pointer moving onto or off the island should be reported.
///
/// `previous` is what was last reported (`None` before the first report). When the
/// island grows under a pointer that is standing still (a pop-up appearing, or the tab
/// coming back after a full-screen app), that is not the user reaching for it, so it
/// only counts as hovering once the pointer moves. Leaving always counts.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn hover_changed(previous: Option<bool>, inside: bool, moved: bool) -> bool {
    if previous == Some(inside) {
        return false;
    }
    !(inside && !moved && previous == Some(false))
}

/// A top-level window as seen by the full-screen check, in physical screen pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StackedWindow {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    /// The window manager reports it as full-screen.
    pub fullscreen: bool,
    /// Always-on-top overlay (picture-in-picture, game overlays, sticky tools). It
    /// only counts when it covers the whole monitor; otherwise the check looks past it.
    pub overlay: bool,
}

impl StackedWindow {
    fn covers(&self, m: &MonitorRect) -> bool {
        self.x <= m.x
            && self.y <= m.y
            && self.x as i64 + self.width as i64 >= m.x as i64 + m.width as i64
            && self.y as i64 + self.height as i64 >= m.y as i64 + m.height as i64
    }

    fn centre_on(&self, m: &MonitorRect) -> bool {
        m.contains(
            self.x as f64 + self.width as f64 / 2.0,
            self.y as f64 + self.height as f64 / 2.0,
        )
    }
}

/// Decides whether the island should step aside: true when the top-most app window on
/// the island's monitor is full-screen (by state, or by covering the whole monitor, which
/// is how many games do it).
///
/// `windows` is ordered top to bottom and must already exclude the island itself and
/// windows that are hidden, minimised, on another workspace, or part of the desktop shell.
/// Looking at the stacking order rather than the focused window keeps the island hidden
/// even after the island itself was clicked.
pub fn fullscreen_on_monitor(windows: &[StackedWindow], monitor: &MonitorRect) -> bool {
    for w in windows.iter().filter(|w| w.centre_on(monitor)) {
        if w.fullscreen || w.covers(monitor) {
            return true;
        }
        if !w.overlay {
            return false;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    const MONITOR: MonitorRect = MonitorRect {
        x: 0,
        y: 0,
        width: 1920,
        height: 1080,
        scale: 1.0,
    };

    fn win(x: i32, y: i32, width: u32, height: u32, fullscreen: bool) -> StackedWindow {
        StackedWindow {
            x,
            y,
            width,
            height,
            fullscreen,
            overlay: false,
        }
    }

    #[test]
    fn small_always_on_top_overlays_are_looked_past() {
        let pip = StackedWindow {
            overlay: true,
            ..win(1500, 800, 400, 225, false)
        };
        assert!(fullscreen_on_monitor(
            &[pip, win(0, 0, 1920, 1080, false)],
            &MONITOR
        ));
        assert!(!fullscreen_on_monitor(
            &[pip, win(100, 100, 800, 600, false)],
            &MONITOR
        ));
        // A normal window is never looked past.
        assert!(!fullscreen_on_monitor(
            &[
                win(1500, 800, 400, 225, false),
                win(0, 0, 1920, 1080, false)
            ],
            &MONITOR
        ));
    }

    #[test]
    fn fullscreen_state_on_top_hides_island() {
        assert!(fullscreen_on_monitor(
            &[win(0, 0, 1920, 1080, true)],
            &MONITOR
        ));
    }

    #[test]
    fn borderless_window_covering_monitor_counts_as_fullscreen() {
        assert!(fullscreen_on_monitor(
            &[win(0, 0, 1920, 1080, false)],
            &MONITOR
        ));
    }

    #[test]
    fn maximised_window_below_top_bar_does_not_hide_island() {
        assert!(!fullscreen_on_monitor(
            &[win(0, 32, 1920, 1048, false)],
            &MONITOR
        ));
    }

    #[test]
    fn only_the_topmost_window_on_this_monitor_matters() {
        let normal_on_top = [win(100, 100, 800, 600, false), win(0, 0, 1920, 1080, true)];
        assert!(!fullscreen_on_monitor(&normal_on_top, &MONITOR));
        let other_monitor_on_top = [win(1920, 0, 1920, 1080, false), win(0, 0, 1920, 1080, true)];
        assert!(fullscreen_on_monitor(&other_monitor_on_top, &MONITOR));
    }

    #[test]
    fn fullscreen_on_another_monitor_is_ignored() {
        assert!(!fullscreen_on_monitor(
            &[win(1920, 0, 1920, 1080, true)],
            &MONITOR
        ));
        assert!(!fullscreen_on_monitor(&[], &MONITOR));
    }

    #[test]
    fn clamps_region_into_window() {
        let r = HitRegion {
            x: -10.0,
            y: -5.0,
            width: 9000.0,
            height: f64::NAN,
        }
        .clamped();
        assert_eq!(
            r,
            HitRegion {
                x: 0.0,
                y: 0.0,
                width: WINDOW_WIDTH,
                height: 0.0
            }
        );
    }

    #[test]
    fn contains_is_half_open() {
        let r = HitRegion {
            x: 10.0,
            y: 0.0,
            width: 100.0,
            height: 30.0,
        };
        assert!(r.contains(10.0, 0.0));
        assert!(r.contains(109.9, 29.9));
        assert!(!r.contains(110.0, 10.0));
        assert!(!r.contains(50.0, 30.0));
    }

    #[test]
    fn centres_window_on_monitor() {
        let m = MonitorRect {
            x: 1920,
            y: 0,
            width: 2560,
            height: 1440,
            scale: 1.0,
        };
        assert_eq!(window_position_for(&m), (1920 + (2560 - 760) / 2, 0));
    }

    #[test]
    fn centres_window_on_scaled_monitor() {
        let m = MonitorRect {
            x: 0,
            y: 0,
            width: 3840,
            height: 2160,
            scale: 2.0,
        };
        assert_eq!(window_position_for(&m), ((3840 - 1520) / 2, 0));
    }

    #[test]
    fn converts_physical_point_to_logical() {
        assert_eq!(
            to_window_logical((1100.0, 40.0), (1000, 0), 2.0),
            (50.0, 20.0)
        );
        assert_eq!(to_window_logical((5.0, 5.0), (0, 0), 0.0), (5.0, 5.0));
    }

    #[test]
    fn hover_needs_a_moving_pointer_but_leaving_does_not() {
        // Reaching for the island.
        assert!(hover_changed(Some(false), true, true));
        // The island grew under a still pointer: not a hover yet.
        assert!(!hover_changed(Some(false), true, false));
        // Leaving always counts, moving or not (the island shrank away).
        assert!(hover_changed(Some(true), false, false));
        assert!(hover_changed(Some(true), false, true));
        // Nothing changed.
        assert!(!hover_changed(Some(true), true, true));
        // First report after start or a move to another screen.
        assert!(hover_changed(None, true, false));
    }
}
