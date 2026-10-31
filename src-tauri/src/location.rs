//! The PC's own location from Windows (Wi-Fi and GPS based), which is far more
//! accurate than guessing from the internet connection. It needs "Let desktop apps
//! access your location" switched on in Windows Settings › Privacy › Location.

use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize)]
pub struct Position {
    pub lat: f64,
    pub lon: f64,
    /// Radius of uncertainty, in metres.
    pub accuracy: f64,
}

/// Asks Windows where the PC is. Fails when location is off or unavailable.
#[tauri::command]
pub async fn precise_location() -> Result<Position, String> {
    tauri::async_runtime::spawn_blocking(platform::locate)
        .await
        .map_err(|e| e.to_string())?
}

#[cfg(windows)]
mod platform {
    use super::Position;
    use windows::Devices::Geolocation::{GeolocationAccessStatus, Geolocator, PositionAccuracy};
    use windows::Foundation::TimeSpan;

    /// 100-nanosecond ticks in a second, the unit of `TimeSpan`.
    const SECOND: i64 = 10_000_000;

    pub fn locate() -> Result<Position, String> {
        let run = || -> windows::core::Result<Result<Position, String>> {
            if Geolocator::RequestAccessAsync()?.get()? != GeolocationAccessStatus::Allowed {
                return Ok(Err("Location is off in Windows settings".into()));
            }
            let locator = Geolocator::new()?;
            locator.SetDesiredAccuracy(PositionAccuracy::Default)?;
            // A fix up to 10 minutes old is fine for weather; give up after 10 s.
            let position = locator
                .GetGeopositionAsyncWithAgeAndTimeout(
                    TimeSpan {
                        Duration: 600 * SECOND,
                    },
                    TimeSpan {
                        Duration: 10 * SECOND,
                    },
                )?
                .get()?;
            let coordinate = position.Coordinate()?;
            let point = coordinate.Point()?.Position()?;
            Ok(Ok(Position {
                lat: point.Latitude,
                lon: point.Longitude,
                accuracy: coordinate.Accuracy()?,
            }))
        };
        run().map_err(|e| format!("Windows couldn't find the location: {}", e.message()))?
    }
}

#[cfg(not(windows))]
mod platform {
    use super::Position;

    pub fn locate() -> Result<Position, String> {
        Err("Precise location needs Windows".into())
    }
}
