//! The PC's own location from Windows (GPS, Wi-Fi and cell based), which is far more
//! accurate than guessing from the internet connection. It needs "Let desktop apps
//! access your location" switched on in Windows Settings › Privacy & security ›
//! Location.

use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize)]
pub struct Position {
    pub lat: f64,
    pub lon: f64,
    /// Radius of uncertainty, in metres.
    pub accuracy: f64,
}

/// Why Windows gave no location, for the Weather tab to explain.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct LocationError {
    /// "off": location is switched off or denied for desktop apps. "unavailable":
    /// it is allowed, but Windows couldn't get a fix right now.
    pub kind: &'static str,
    pub message: String,
}

impl LocationError {
    #[cfg_attr(not(windows), allow(dead_code))]
    fn off(message: &str) -> Self {
        Self {
            kind: "off",
            message: message.into(),
        }
    }

    fn unavailable(message: impl Into<String>) -> Self {
        Self {
            kind: "unavailable",
            message: message.into(),
        }
    }
}

/// Asks Windows where the PC is, as precisely as it can.
#[tauri::command]
pub async fn precise_location() -> Result<Position, LocationError> {
    let found = tauri::async_runtime::spawn_blocking(platform::locate)
        .await
        .map_err(|e| LocationError::unavailable(e.to_string()))?;
    match &found {
        Ok(p) => log::info!("location: Windows found the PC within {:.0} m", p.accuracy),
        Err(e) => log::info!("location: {} ({})", e.message, e.kind),
    }
    found
}

/// Opens Windows Settings › Privacy & security › Location.
#[tauri::command]
pub fn open_location_settings() {
    platform::open_settings();
}

#[cfg(windows)]
mod platform {
    use super::{LocationError, Position};
    use windows::core::w;
    use windows::Devices::Geolocation::{
        GeolocationAccessStatus, Geolocator, Geoposition, PositionAccuracy, PositionSource,
        PositionStatus,
    };
    use windows::Foundation::TimeSpan;
    use windows::Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL};

    /// 100-nanosecond ticks in a second, the unit of `TimeSpan`.
    const SECOND: i64 = 10_000_000;

    const OFF: &str = "Location is off for desktop apps in Windows settings";

    fn span(seconds: i64) -> TimeSpan {
        TimeSpan {
            Duration: seconds * SECOND,
        }
    }

    fn source_name(source: PositionSource) -> &'static str {
        match source {
            PositionSource::Satellite => "GPS",
            PositionSource::WiFi => "Wi-Fi",
            PositionSource::Cellular => "mobile network",
            PositionSource::IPAddress => "IP address",
            _ => "other",
        }
    }

    fn to_position(fix: &Geoposition) -> windows::core::Result<Position> {
        let coordinate = fix.Coordinate()?;
        let point = coordinate.Point()?.Position()?;
        let accuracy = coordinate.Accuracy()?;
        if let Ok(source) = coordinate.PositionSource() {
            log::info!(
                "location: fix from {} within {accuracy:.0} m",
                source_name(source)
            );
        }
        Ok(Position {
            lat: point.Latitude,
            lon: point.Longitude,
            accuracy,
        })
    }

    pub fn locate() -> Result<Position, LocationError> {
        let err = |e: windows::core::Error| {
            LocationError::unavailable(format!(
                "Windows couldn't find the location: {}",
                e.message()
            ))
        };
        let access = Geolocator::RequestAccessAsync()
            .and_then(|op| op.get())
            .map_err(err)?;
        if access != GeolocationAccessStatus::Allowed {
            return Err(LocationError::off(OFF));
        }
        let locator = Geolocator::new().map_err(err)?;
        if let Ok(PositionStatus::Disabled | PositionStatus::NotAvailable) =
            locator.LocationStatus()
        {
            return Err(LocationError::off(OFF));
        }
        // High accuracy lets Windows use GPS and a fresh Wi-Fi scan instead of a
        // cached, city-sized estimate. Accept a fix up to a minute old; wait up to 15 s.
        let precise = locator
            .SetDesiredAccuracy(PositionAccuracy::High)
            .and_then(|_| locator.GetGeopositionAsyncWithAgeAndTimeout(span(60), span(15)))
            .and_then(|op| op.get())
            .and_then(|fix| to_position(&fix));
        match precise {
            Ok(p) => Ok(p),
            Err(e) => {
                log::info!(
                    "location: no high-accuracy fix ({}), asking for any recent one",
                    e.message()
                );
                // No GPS and no Wi-Fi fix in time: take whatever Windows knows.
                locator
                    .SetDesiredAccuracy(PositionAccuracy::Default)
                    .and_then(|_| locator.GetGeopositionAsyncWithAgeAndTimeout(span(600), span(10)))
                    .and_then(|op| op.get())
                    .and_then(|fix| to_position(&fix))
                    .map_err(err)
            }
        }
    }

    pub fn open_settings() {
        unsafe {
            ShellExecuteW(
                None,
                w!("open"),
                w!("ms-settings:privacy-location"),
                None,
                None,
                SW_SHOWNORMAL,
            );
        }
    }
}

#[cfg(not(windows))]
mod platform {
    use super::{LocationError, Position};

    pub fn locate() -> Result<Position, LocationError> {
        Err(LocationError::unavailable("Precise location needs Windows"))
    }

    pub fn open_settings() {}
}
