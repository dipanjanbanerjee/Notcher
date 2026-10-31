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

#[cfg(test)]
mod tests {
    use super::*;

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
}
