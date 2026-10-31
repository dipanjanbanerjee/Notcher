//! Quick notes ("Jot") commands.

use crate::clipboard::now_ms;
use crate::db::{self, Db, Note, NoteInput};
use tauri::State;

/// Longest note accepted, to keep the database small.
const MAX_NOTE_BYTES: usize = 256 * 1024;

#[tauri::command]
pub fn notes_list(db: State<'_, Db>) -> Result<Vec<Note>, String> {
    db.with(db::list_notes)
}

#[tauri::command]
pub fn note_save(db: State<'_, Db>, note: NoteInput) -> Result<Note, String> {
    if note.body.len() > MAX_NOTE_BYTES {
        return Err("note is too long".into());
    }
    db.with(|c| db::save_note(c, &note, now_ms()))
}

#[tauri::command]
pub fn note_delete(db: State<'_, Db>, id: i64) -> Result<(), String> {
    db.with(|c| db::delete_note(c, id))
}
