//! Clipboard history.
//!
//! A platform watcher notices new clipboard content and hands it to [`capture`],
//! which stores it (text in SQLite, images as PNG files next to it) and tells the UI.
//! Content that password managers mark as sensitive is never read.

mod image;
#[cfg(target_os = "linux")]
mod linux_x11;
mod poll;

use crate::db::{self, Db, NewClip};
use image::{fnv1a, Rgba};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager, Runtime, State};

pub const ADDED_EVENT: &str = "clips://added";
/// Text longer than this is not kept in history.
const MAX_TEXT_BYTES: usize = 1024 * 1024;

pub enum Content {
    Text(String),
    Image(Rgba),
}

pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn images_dir<R: Runtime>(app: &AppHandle<R>) -> Option<PathBuf> {
    let dir = app.path().app_data_dir().ok()?.join("clips");
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

fn remove_files(paths: impl IntoIterator<Item = String>) {
    for p in paths {
        let _ = std::fs::remove_file(p);
    }
}

/// Stores newly copied content and notifies the UI.
pub fn capture<R: Runtime>(app: &AppHandle<R>, content: Content) {
    if !crate::settings::clipboard_history(app) {
        return;
    }
    let db = app.state::<Db>();
    let now = now_ms();
    let result = match content {
        Content::Text(text) => {
            if text.trim().is_empty() || text.len() > MAX_TEXT_BYTES {
                return;
            }
            let hash = fnv1a(&[b"text", text.as_bytes()]);
            let clip = NewClip {
                kind: "text",
                text: Some(&text),
                image_path: None,
                thumb: None,
                width: 0,
                height: 0,
                hash: &hash,
            };
            db.with(|c| db::upsert_clip(c, &clip, now))
        }
        Content::Image(img) => {
            let hash = fnv1a(&[
                b"image",
                &img.width.to_le_bytes(),
                &img.height.to_le_bytes(),
                &img.pixels,
            ]);
            let Some(dir) = images_dir(app) else { return };
            let path = dir.join(format!("{hash}.png"));
            if !path.exists() {
                let Some(png) = img.to_png() else { return };
                if let Err(err) = std::fs::write(&path, png) {
                    log::warn!("could not save clipboard image: {err}");
                    return;
                }
            }
            let thumb = img.thumbnail_data_url();
            let path_str = path.to_string_lossy().to_string();
            let clip = NewClip {
                kind: "image",
                text: None,
                image_path: Some(&path_str),
                thumb: thumb.as_deref(),
                width: img.width,
                height: img.height,
                hash: &hash,
            };
            db.with(|c| db::upsert_clip(c, &clip, now))
        }
    };
    match result {
        Ok((clip, pruned)) => {
            remove_files(pruned);
            let _ = app.emit(ADDED_EVENT, &clip);
        }
        Err(err) => log::warn!("could not store clip: {err}"),
    }
}

/// The app's long-lived clipboard handle, used to copy items back. Keeping one alive
/// matters on Linux, where the owning process must keep serving the data.
static WRITER: Mutex<Option<arboard::Clipboard>> = Mutex::new(None);

fn write_clipboard(content: Content) -> Result<(), String> {
    let mut guard = WRITER.lock().map_err(|_| "clipboard lock poisoned")?;
    if guard.is_none() {
        *guard = Some(arboard::Clipboard::new().map_err(|e| e.to_string())?);
    }
    let cb = guard.as_mut().expect("clipboard initialised above");
    match content {
        Content::Text(text) => cb.set_text(text),
        Content::Image(img) => cb.set_image(arboard::ImageData {
            width: img.width as usize,
            height: img.height as usize,
            bytes: img.pixels.into(),
        }),
    }
    .map_err(|e| e.to_string())
}

pub fn start<R: Runtime>(app: AppHandle<R>) {
    #[cfg(target_os = "linux")]
    {
        // X11 sessions, and GNOME on Wayland where Notcher runs through XWayland
        // (whose clipboard is kept in sync with the Wayland one).
        let forced_x11 = std::env::var("GDK_BACKEND").is_ok_and(|b| b == "x11");
        let not_wayland = std::env::var("XDG_SESSION_TYPE").map_or(true, |t| t != "wayland");
        let x11 = std::env::var_os("DISPLAY").is_some() && (forced_x11 || not_wayland);
        if x11 && linux_x11::spawn(app.clone()) {
            return;
        }
    }
    poll::spawn(app);
}

// ------------------------------------------------------------------ commands

#[tauri::command]
pub fn clips_list(db: State<'_, Db>, query: Option<String>) -> Result<Vec<db::Clip>, String> {
    db.with(|c| db::list_clips(c, query.as_deref().filter(|q| !q.is_empty()), 100))
}

#[tauri::command]
pub async fn clip_copy<R: Runtime>(app: AppHandle<R>, id: i64) -> Result<(), String> {
    let payload = app.state::<Db>().with(|c| db::clip_payload(c, id))?;
    let content = match payload {
        Some((Some(text), _)) => Content::Text(text),
        Some((None, Some(path))) => {
            let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
            Content::Image(Rgba::from_png(&bytes).ok_or("could not read the saved image")?)
        }
        _ => return Err("clip not found".into()),
    };
    tauri::async_runtime::spawn_blocking(move || write_clipboard(content))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn clip_set_pinned(db: State<'_, Db>, id: i64, pinned: bool) -> Result<(), String> {
    db.with(|c| db::set_clip_pinned(c, id, pinned))
}

#[tauri::command]
pub fn clip_delete(db: State<'_, Db>, id: i64) -> Result<(), String> {
    let path = db.with(|c| db::delete_clip(c, id))?;
    remove_files(path);
    Ok(())
}

#[tauri::command]
pub fn clips_clear(db: State<'_, Db>) -> Result<(), String> {
    let paths = db.with(db::clear_clips)?;
    remove_files(paths);
    Ok(())
}
