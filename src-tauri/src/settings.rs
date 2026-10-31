//! Settings shared with the web UI through `tauri-plugin-store` (`settings.json`).
//! The web UI owns the full schema; Rust only reads the keys it needs.

use tauri::{AppHandle, Runtime};
use tauri_plugin_store::StoreExt;

pub const STORE_FILE: &str = "settings.json";
pub const DEFAULT_SHORTCUT: &str = "CommandOrControl+Alt+Space";

fn get<R: Runtime>(app: &AppHandle<R>, key: &str) -> Option<serde_json::Value> {
    app.store(STORE_FILE).ok()?.get(key)
}

/// Move the island to whichever monitor the cursor is on.
pub fn follow_cursor<R: Runtime>(app: &AppHandle<R>) -> bool {
    get(app, "followCursor")
        .and_then(|v| v.as_bool())
        .unwrap_or(true)
}

/// A boolean setting written by the web UI.
pub fn flag<R: Runtime>(app: &AppHandle<R>, key: &str, default: bool) -> bool {
    get(app, key).and_then(|v| v.as_bool()).unwrap_or(default)
}

/// A text setting written by the web UI.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn text<R: Runtime>(app: &AppHandle<R>, key: &str, default: &str) -> String {
    get(app, key)
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_else(|| default.to_owned())
}

/// Keep a history of copied text and images.
pub fn clipboard_history<R: Runtime>(app: &AppHandle<R>) -> bool {
    get(app, "clipboardHistory")
        .and_then(|v| v.as_bool())
        .unwrap_or(true)
}

/// Global shortcut that opens and closes the island.
pub fn shortcut<R: Runtime>(app: &AppHandle<R>) -> String {
    get(app, "shortcut")
        .and_then(|v| v.as_str().map(str::to_owned))
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_SHORTCUT.to_owned())
}
