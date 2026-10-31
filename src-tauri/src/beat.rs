//! Output loudness for the beat glow under the island.
//!
//! Reads Windows' peak meter for the default speakers: the same level the volume
//! mixer shows. Nothing is recorded; it is one number, read about 30 times a second
//! and only while music is playing and the glow is visible.

use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Runtime};

#[cfg_attr(not(windows), allow(dead_code))]
pub const LEVEL_EVENT: &str = "audio://level";

static ACTIVE: AtomicBool = AtomicBool::new(false);

#[tauri::command]
pub fn set_beat_active(active: bool) {
    ACTIVE.store(active, Ordering::Relaxed);
}

pub fn start<R: Runtime>(app: AppHandle<R>) {
    #[cfg(windows)]
    platform::spawn(app);
    #[cfg(not(windows))]
    let _ = app;
}

#[cfg(windows)]
mod platform {
    use super::{ACTIVE, LEVEL_EVENT};
    use std::sync::atomic::Ordering;
    use std::time::{Duration, Instant};
    use tauri::{AppHandle, Emitter, Runtime};
    use windows::Win32::{
        Media::Audio::{
            eConsole, eRender, Endpoints::IAudioMeterInformation, IMMDeviceEnumerator,
            MMDeviceEnumerator,
        },
        System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_MULTITHREADED},
    };

    const FRAME: Duration = Duration::from_millis(33);
    const IDLE: Duration = Duration::from_millis(250);
    /// The default output device can change (headphones plugged in), so the meter
    /// is looked up again every few seconds.
    const REFRESH_DEVICE: Duration = Duration::from_secs(4);

    pub fn spawn<R: Runtime>(app: AppHandle<R>) {
        let spawned = std::thread::Builder::new()
            .name("notcher-beat".into())
            .spawn(move || run(app));
        if let Err(err) = spawned {
            log::error!("could not start beat meter: {err}");
        }
    }

    fn meter() -> windows::core::Result<IAudioMeterInformation> {
        unsafe {
            let enumerator: IMMDeviceEnumerator =
                CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
            let device = enumerator.GetDefaultAudioEndpoint(eRender, eConsole)?;
            device.Activate(CLSCTX_ALL, None)
        }
    }

    fn run<R: Runtime>(app: AppHandle<R>) {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        }
        let mut current: Option<(IAudioMeterInformation, Instant)> = None;
        let mut last_sent = -1.0f32;
        loop {
            if !ACTIVE.load(Ordering::Relaxed) {
                current = None;
                if last_sent != 0.0 {
                    last_sent = 0.0;
                    let _ = app.emit(LEVEL_EVENT, 0.0f32);
                }
                std::thread::sleep(IDLE);
                continue;
            }
            let stale = match &current {
                Some((_, at)) => at.elapsed() >= REFRESH_DEVICE,
                None => true,
            };
            if stale {
                current = meter().ok().map(|m| (m, Instant::now()));
            }
            let level = current
                .as_ref()
                .and_then(|(m, _)| unsafe { m.GetPeakValue() }.ok());
            match level {
                Some(v) => {
                    let v = v.clamp(0.0, 1.0);
                    if (v - last_sent).abs() > 0.004 {
                        last_sent = v;
                        let _ = app.emit(LEVEL_EVENT, v);
                    }
                }
                None => current = None,
            }
            std::thread::sleep(FRAME);
        }
    }
}
