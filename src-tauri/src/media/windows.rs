//! System Media Transport Controls (SMTC) backend for Windows.

use super::{cached_art, data_url, MediaAction, NowPlaying};
use std::time::{Duration, Instant};
use windows::{
    Media::Control::{
        GlobalSystemMediaTransportControlsSession as Session,
        GlobalSystemMediaTransportControlsSessionManager as Manager,
        GlobalSystemMediaTransportControlsSessionPlaybackStatus as Status,
    },
    Storage::Streams::{DataReader, IRandomAccessStreamReference},
    Win32::System::WinRT::{RoInitialize, RO_INIT_MULTITHREADED},
};

/// After a track change, players (browsers especially) update the title first and
/// the artwork a moment later, sometimes showing their own app icon in between. The
/// cover is re-read on every poll for this long before it is cached.
const ART_SETTLE: Duration = Duration::from_secs(12);

pub struct Reader {
    manager: Manager,
    track_changed_at: Instant,
}

impl Reader {
    pub fn new() -> Result<Self, String> {
        // The watcher runs on its own thread, which needs a WinRT apartment.
        // An "already initialized" result is fine, so it is ignored.
        unsafe {
            let _ = RoInitialize(RO_INIT_MULTITHREADED);
        }
        let manager = Manager::RequestAsync()
            .and_then(|op| op.get())
            .map_err(|e| e.to_string())?;
        Ok(Self {
            manager,
            track_changed_at: Instant::now(),
        })
    }

    pub fn read(&mut self, previous: Option<&NowPlaying>) -> Option<NowPlaying> {
        let session = self.manager.GetCurrentSession().ok()?;
        let (mut now, props) = snapshot(&session).ok().flatten()?;
        let cached = cached_art(previous, &now);
        if cached.is_none() {
            self.track_changed_at = Instant::now();
        }
        let settled = self.track_changed_at.elapsed() >= ART_SETTLE;
        now.art = match cached {
            Some(art) if settled => art,
            cached => props
                .Thumbnail()
                .ok()
                .and_then(|thumb| read_thumbnail(&thumb).ok())
                .flatten()
                // A failed read keeps the cover we already had for this track.
                .or(cached.flatten()),
        };
        Some(now)
    }
}

type Properties = windows::Media::Control::GlobalSystemMediaTransportControlsSessionMediaProperties;

/// Everything except the cover art, plus the properties to read the art from.
fn snapshot(session: &Session) -> windows::core::Result<Option<(NowPlaying, Properties)>> {
    let props = session.TryGetMediaPropertiesAsync()?.get()?;
    let title = props.Title()?.to_string_lossy().trim().to_string();
    if title.is_empty() {
        return Ok(None);
    }
    let playing = session
        .GetPlaybackInfo()
        .and_then(|info| info.PlaybackStatus())
        .map(|status| status == Status::Playing)
        .unwrap_or(false);
    // Timeline values are TimeSpans in 100 ns ticks.
    let (position_ms, duration_ms) = session
        .GetTimelineProperties()
        .and_then(|t| Ok((t.Position()?.Duration, t.EndTime()?.Duration)))
        .map(|(pos, end)| ((pos.max(0) / 10_000) as u64, (end.max(0) / 10_000) as u64))
        .unwrap_or((0, 0));

    let now = NowPlaying {
        title,
        artist: props.Artist()?.to_string_lossy(),
        album: props.AlbumTitle()?.to_string_lossy(),
        art: None,
        playing,
        position_ms,
        duration_ms,
        source: friendly_source(&session.SourceAppUserModelId()?.to_string_lossy()),
    };
    Ok(Some((now, props)))
}

fn read_thumbnail(thumb: &IRandomAccessStreamReference) -> windows::core::Result<Option<String>> {
    let stream = thumb.OpenReadAsync()?.get()?;
    let size = stream.Size()? as u32;
    if size == 0 {
        return Ok(None);
    }
    let reader = DataReader::CreateDataReader(&stream.GetInputStreamAt(0)?)?;
    reader.LoadAsync(size)?.get()?;
    let mut bytes = vec![0u8; size as usize];
    reader.ReadBytes(&mut bytes)?;
    Ok(data_url(&bytes))
}

/// Turns an app id such as `Spotify.exe` or `Microsoft.ZuneMusic_8wekyb3d8bbwe!Microsoft.ZuneMusic`
/// into something readable.
fn friendly_source(app_id: &str) -> String {
    let base = app_id.split('!').next_back().unwrap_or(app_id);
    let base = base.trim_end_matches(".exe");
    let base = base.rsplit('.').next().unwrap_or(base);
    match base.to_ascii_lowercase().as_str() {
        "zunemusic" => "Media Player".into(),
        "msedge" => "Edge".into(),
        "chrome" => "Chrome".into(),
        "firefox" => "Firefox".into(),
        _ => base.to_string(),
    }
}

pub fn send(action: MediaAction) -> Result<(), String> {
    let run = || -> windows::core::Result<bool> {
        let session = Manager::RequestAsync()?.get()?.GetCurrentSession()?;
        match action {
            MediaAction::PlayPause => session.TryTogglePlayPauseAsync()?.get(),
            MediaAction::Next => session.TrySkipNextAsync()?.get(),
            MediaAction::Previous => session.TrySkipPreviousAsync()?.get(),
        }
    };
    match run() {
        Ok(true) => Ok(()),
        Ok(false) => Err("the player ignored the command".into()),
        Err(err) => Err(err.to_string()),
    }
}
