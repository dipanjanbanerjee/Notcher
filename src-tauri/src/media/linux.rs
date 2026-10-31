//! MPRIS (D-Bus) backend for Linux.

use super::{cached_art, data_url, MediaAction, NowPlaying};
use mpris::{PlaybackStatus, Player, PlayerFinder};

pub struct Reader {
    finder: PlayerFinder,
}

impl Reader {
    pub fn new() -> Result<Self, String> {
        PlayerFinder::new()
            .map(|finder| Self { finder })
            .map_err(|e| e.to_string())
    }

    pub fn read(&mut self, previous: Option<&NowPlaying>) -> Option<NowPlaying> {
        let player = self.finder.find_active().ok()?;
        snapshot(&player, previous)
    }
}

fn snapshot(player: &Player, previous: Option<&NowPlaying>) -> Option<NowPlaying> {
    let metadata = player.get_metadata().ok()?;
    let title = metadata.title().unwrap_or_default().trim().to_string();
    if title.is_empty() {
        return None;
    }
    let mut now = NowPlaying {
        title,
        artist: metadata.artists().map(|a| a.join(", ")).unwrap_or_default(),
        album: metadata.album_name().unwrap_or_default().to_string(),
        art: None,
        playing: matches!(player.get_playback_status(), Ok(PlaybackStatus::Playing)),
        position_ms: player
            .get_position()
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0),
        duration_ms: metadata.length().map(|d| d.as_millis() as u64).unwrap_or(0),
        source: player.identity().to_string(),
    };
    now.art = match cached_art(previous, &now) {
        Some(art) => art,
        None => metadata.art_url().and_then(load_art),
    };
    Some(now)
}

/// Local files become `data:` URLs; web URLs are passed through for the UI to load.
fn load_art(art_url: &str) -> Option<String> {
    if art_url.starts_with("https://") {
        return Some(art_url.to_string());
    }
    let path = url::Url::parse(art_url).ok()?.to_file_path().ok()?;
    let bytes = std::fs::read(path).ok()?;
    data_url(&bytes)
}

pub fn send(action: MediaAction) -> Result<(), String> {
    let player = PlayerFinder::new()
        .map_err(|e| e.to_string())?
        .find_active()
        .map_err(|e| e.to_string())?;
    let result = match action {
        MediaAction::PlayPause => player.play_pause(),
        MediaAction::Next => player.next(),
        MediaAction::Previous => player.previous(),
    };
    result.map_err(|e| e.to_string())
}
