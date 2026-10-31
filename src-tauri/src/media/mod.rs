//! "Now playing" information and media controls.
//!
//! Each platform backend polls the system media service (SMTC on Windows, MPRIS on
//! Linux) on its own thread and emits [`MEDIA_EVENT`] whenever something changes.

use serde::{Deserialize, Serialize};
use std::{sync::Mutex, thread, time::Duration};
use tauri::{AppHandle, Emitter, Runtime};

#[cfg(target_os = "linux")]
mod linux;
#[cfg(windows)]
mod windows;

#[cfg(target_os = "linux")]
use linux as backend;
#[cfg(windows)]
use windows as backend;

/// Event name used to push [`NowPlaying`] updates to the web UI.
pub const MEDIA_EVENT: &str = "media://update";

#[derive(Debug, Clone, PartialEq, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct NowPlaying {
    pub title: String,
    pub artist: String,
    pub album: String,
    /// `data:` URL or `https:` URL of the cover art, if any.
    pub art: Option<String>,
    pub playing: bool,
    /// Playback position and track length in milliseconds (0 when unknown).
    pub position_ms: u64,
    pub duration_ms: u64,
    /// Friendly name of the player app, e.g. "Spotify".
    pub source: String,
}

impl NowPlaying {
    /// Two snapshots describe the same track (used to avoid re-reading cover art).
    pub fn same_track(&self, other: &NowPlaying) -> bool {
        self.title == other.title && self.artist == other.artist && self.album == other.album
    }

    /// Whether a change is worth sending to the UI. Position drifts naturally while
    /// playing, so only jumps larger than two seconds count as a change.
    pub fn differs_meaningfully(&self, other: &NowPlaying) -> bool {
        !self.same_track(other)
            || self.playing != other.playing
            || self.duration_ms != other.duration_ms
            || self.art != other.art
            || self.position_ms.abs_diff(other.position_ms) > 2_000
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MediaAction {
    PlayPause,
    Next,
    Previous,
}

static LATEST: Mutex<Option<NowPlaying>> = Mutex::new(None);

/// How often the system media service is polled.
const POLL_INTERVAL: Duration = Duration::from_millis(1000);

/// Starts the background watcher thread for the current platform.
pub fn start<R: Runtime>(app: AppHandle<R>) {
    let spawned = thread::Builder::new()
        .name("notcher-media".into())
        .spawn(move || {
            let mut reader = match backend::Reader::new() {
                Ok(reader) => reader,
                Err(err) => {
                    log::warn!("media watcher unavailable: {err}");
                    return;
                }
            };
            let mut last: Option<NowPlaying> = None;
            loop {
                let current = reader.read(last.as_ref());
                let changed = match (&last, &current) {
                    (None, None) => false,
                    (Some(a), Some(b)) => a.differs_meaningfully(b),
                    _ => true,
                };
                if let Ok(mut latest) = LATEST.lock() {
                    *latest = current.clone();
                }
                if changed {
                    if let Err(err) = app.emit(MEDIA_EVENT, &current) {
                        log::warn!("failed to emit media update: {err}");
                    }
                }
                last = current;
                thread::sleep(POLL_INTERVAL);
            }
        });
    if let Err(err) = spawned {
        log::error!("could not start media watcher: {err}");
    }
}

/// Reuses the previous cover art when the track has not changed, so it is not
/// re-read and re-encoded on every poll.
pub(crate) fn cached_art(
    previous: Option<&NowPlaying>,
    current: &NowPlaying,
) -> Option<Option<String>> {
    previous
        .filter(|p| p.same_track(current))
        .map(|p| p.art.clone())
}

/// Encodes raw image bytes as a `data:` URL, sniffing the format from its header.
pub(crate) fn data_url(bytes: &[u8]) -> Option<String> {
    use base64::Engine;
    const MAX_ART_BYTES: usize = 4 * 1024 * 1024;
    if bytes.is_empty() || bytes.len() > MAX_ART_BYTES {
        return None;
    }
    let mime = if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        "image/png"
    } else if bytes.starts_with(&[0xFF, 0xD8]) {
        "image/jpeg"
    } else if bytes.len() > 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        "image/webp"
    } else if bytes.starts_with(b"GIF8") {
        "image/gif"
    } else if bytes.starts_with(b"BM") {
        "image/bmp"
    } else {
        return None;
    };
    Some(format!(
        "data:{mime};base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    ))
}

/// Runs off the main thread: the platform calls block until the player answers.
#[tauri::command]
pub async fn media_command(action: MediaAction) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || backend::send(action))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn now_playing() -> Option<NowPlaying> {
    LATEST.lock().ok().and_then(|latest| latest.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track() -> NowPlaying {
        NowPlaying {
            title: "Song".into(),
            artist: "Artist".into(),
            album: "Album".into(),
            playing: true,
            position_ms: 10_000,
            duration_ms: 200_000,
            ..Default::default()
        }
    }

    #[test]
    fn small_position_drift_is_not_a_change() {
        let a = track();
        let b = NowPlaying {
            position_ms: 11_500,
            ..track()
        };
        assert!(!a.differs_meaningfully(&b));
    }

    #[test]
    fn data_url_sniffs_format_and_rejects_unknown() {
        assert!(data_url(&[0x89, b'P', b'N', b'G', 0, 0])
            .unwrap()
            .starts_with("data:image/png;base64,"));
        assert!(data_url(&[0xFF, 0xD8, 0xFF])
            .unwrap()
            .starts_with("data:image/jpeg;base64,"));
        assert_eq!(data_url(b"hello"), None);
        assert_eq!(data_url(&[]), None);
    }

    #[test]
    fn art_is_reused_only_for_same_track() {
        let prev = NowPlaying {
            art: Some("data:x".into()),
            ..track()
        };
        assert_eq!(
            cached_art(Some(&prev), &track()),
            Some(Some("data:x".into()))
        );
        let other = NowPlaying {
            title: "Other".into(),
            ..track()
        };
        assert_eq!(cached_art(Some(&prev), &other), None);
        assert_eq!(cached_art(None, &track()), None);
    }

    #[test]
    fn seek_pause_and_track_change_are_changes() {
        let a = track();
        assert!(a.differs_meaningfully(&NowPlaying {
            position_ms: 60_000,
            ..track()
        }));
        assert!(a.differs_meaningfully(&NowPlaying {
            playing: false,
            ..track()
        }));
        assert!(a.differs_meaningfully(&NowPlaying {
            title: "Other".into(),
            ..track()
        }));
    }
}
