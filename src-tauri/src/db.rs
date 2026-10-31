//! Local storage for clipboard history and notes (SQLite in the app data folder).

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Mutex;

pub struct Db(pub Mutex<Connection>);

/// Unpinned clipboard items beyond this many are removed, oldest first.
pub const MAX_CLIPS: i64 = 200;

impl Db {
    pub fn open(path: &Path) -> rusqlite::Result<Self> {
        let conn = Connection::open(path)?;
        Self::init(conn)
    }

    #[cfg(test)]
    pub fn in_memory() -> Self {
        Self::init(Connection::open_in_memory().unwrap()).unwrap()
    }

    fn init(conn: Connection) -> rusqlite::Result<Self> {
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA foreign_keys = ON;
             CREATE TABLE IF NOT EXISTS clips (
               id         INTEGER PRIMARY KEY,
               kind       TEXT NOT NULL,
               text       TEXT,
               image_path TEXT,
               thumb      TEXT,
               width      INTEGER NOT NULL DEFAULT 0,
               height     INTEGER NOT NULL DEFAULT 0,
               hash       TEXT NOT NULL UNIQUE,
               created_at INTEGER NOT NULL,
               pinned     INTEGER NOT NULL DEFAULT 0
             );
             CREATE INDEX IF NOT EXISTS clips_order ON clips (pinned DESC, created_at DESC);
             CREATE TABLE IF NOT EXISTS notes (
               id         INTEGER PRIMARY KEY,
               body       TEXT NOT NULL,
               pinned     INTEGER NOT NULL DEFAULT 0,
               updated_at INTEGER NOT NULL
             );",
        )?;
        Ok(Self(Mutex::new(conn)))
    }

    pub fn with<T>(&self, f: impl FnOnce(&Connection) -> rusqlite::Result<T>) -> Result<T, String> {
        let conn = self
            .0
            .lock()
            .map_err(|_| "database lock poisoned".to_string())?;
        f(&conn).map_err(|e| e.to_string())
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Clip {
    pub id: i64,
    pub kind: String,
    pub text: Option<String>,
    /// Thumbnail as a data URL.
    pub image: Option<String>,
    pub width: u32,
    pub height: u32,
    pub created_at: i64,
    pub pinned: bool,
}

/// What a new clipboard entry carries before it is stored.
pub struct NewClip<'a> {
    pub kind: &'a str,
    pub text: Option<&'a str>,
    pub image_path: Option<&'a str>,
    pub thumb: Option<&'a str>,
    pub width: u32,
    pub height: u32,
    pub hash: &'a str,
}

const CLIP_COLUMNS: &str = "id, kind, text, thumb, width, height, created_at, pinned";

fn clip_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Clip> {
    Ok(Clip {
        id: row.get(0)?,
        kind: row.get(1)?,
        text: row.get(2)?,
        image: row.get(3)?,
        width: row.get(4)?,
        height: row.get(5)?,
        created_at: row.get(6)?,
        pinned: row.get::<_, i64>(7)? != 0,
    })
}

/// Stores a clip, or moves an identical existing one to the top.
/// Returns the clip and any image files that were pruned and can be deleted.
pub fn upsert_clip(
    conn: &Connection,
    clip: &NewClip<'_>,
    now: i64,
) -> rusqlite::Result<(Clip, Vec<String>)> {
    conn.execute(
        "INSERT INTO clips (kind, text, image_path, thumb, width, height, hash, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
         ON CONFLICT(hash) DO UPDATE SET created_at = excluded.created_at",
        params![
            clip.kind,
            clip.text,
            clip.image_path,
            clip.thumb,
            clip.width,
            clip.height,
            clip.hash,
            now
        ],
    )?;
    let stored = conn.query_row(
        &format!("SELECT {CLIP_COLUMNS} FROM clips WHERE hash = ?1"),
        [clip.hash],
        clip_from_row,
    )?;
    let pruned = prune_clips(conn)?;
    Ok((stored, pruned))
}

fn prune_clips(conn: &Connection) -> rusqlite::Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT id, image_path FROM clips WHERE pinned = 0
         ORDER BY created_at DESC LIMIT -1 OFFSET ?1",
    )?;
    let old: Vec<(i64, Option<String>)> = stmt
        .query_map([MAX_CLIPS], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;
    for (id, _) in &old {
        conn.execute("DELETE FROM clips WHERE id = ?1", [id])?;
    }
    Ok(old.into_iter().filter_map(|(_, p)| p).collect())
}

pub fn list_clips(
    conn: &Connection,
    query: Option<&str>,
    limit: i64,
) -> rusqlite::Result<Vec<Clip>> {
    let pattern = query.map(|q| {
        format!(
            "%{}%",
            q.replace('\\', "\\\\")
                .replace('%', "\\%")
                .replace('_', "\\_")
        )
    });
    let mut stmt = conn.prepare(&format!(
        "SELECT {CLIP_COLUMNS} FROM clips
         WHERE ?1 IS NULL OR text LIKE ?1 ESCAPE '\\'
         ORDER BY pinned DESC, created_at DESC LIMIT ?2"
    ))?;
    let rows = stmt.query_map(params![pattern, limit], clip_from_row)?;
    rows.collect()
}

/// Text or the stored image path of a clip, for copying it back.
pub fn clip_payload(
    conn: &Connection,
    id: i64,
) -> rusqlite::Result<Option<(Option<String>, Option<String>)>> {
    conn.query_row(
        "SELECT text, image_path FROM clips WHERE id = ?1",
        [id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )
    .optional()
}

pub fn set_clip_pinned(conn: &Connection, id: i64, pinned: bool) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE clips SET pinned = ?2 WHERE id = ?1",
        params![id, pinned as i64],
    )?;
    Ok(())
}

/// Deletes a clip and returns its image file, if any.
pub fn delete_clip(conn: &Connection, id: i64) -> rusqlite::Result<Option<String>> {
    let path: Option<String> = conn
        .query_row("SELECT image_path FROM clips WHERE id = ?1", [id], |r| {
            r.get(0)
        })
        .optional()?
        .flatten();
    conn.execute("DELETE FROM clips WHERE id = ?1", [id])?;
    Ok(path)
}

/// Removes every unpinned clip and returns their image files.
pub fn clear_clips(conn: &Connection) -> rusqlite::Result<Vec<String>> {
    let mut stmt =
        conn.prepare("SELECT image_path FROM clips WHERE pinned = 0 AND image_path IS NOT NULL")?;
    let paths: Vec<String> = stmt
        .query_map([], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    conn.execute("DELETE FROM clips WHERE pinned = 0", [])?;
    Ok(paths)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Note {
    pub id: i64,
    pub body: String,
    pub pinned: bool,
    pub updated_at: i64,
}

#[derive(Debug, Deserialize)]
pub struct NoteInput {
    pub id: Option<i64>,
    pub body: String,
    pub pinned: bool,
}

pub fn list_notes(conn: &Connection) -> rusqlite::Result<Vec<Note>> {
    let mut stmt = conn.prepare(
        "SELECT id, body, pinned, updated_at FROM notes ORDER BY pinned DESC, updated_at DESC",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(Note {
            id: r.get(0)?,
            body: r.get(1)?,
            pinned: r.get::<_, i64>(2)? != 0,
            updated_at: r.get(3)?,
        })
    })?;
    rows.collect()
}

pub fn save_note(conn: &Connection, input: &NoteInput, now: i64) -> rusqlite::Result<Note> {
    let id = match input.id {
        Some(id) => {
            let changed = conn.execute(
                "UPDATE notes SET body = ?2, pinned = ?3, updated_at = ?4 WHERE id = ?1",
                params![id, input.body, input.pinned as i64, now],
            )?;
            if changed == 0 {
                return Err(rusqlite::Error::QueryReturnedNoRows);
            }
            id
        }
        None => {
            conn.execute(
                "INSERT INTO notes (body, pinned, updated_at) VALUES (?1, ?2, ?3)",
                params![input.body, input.pinned as i64, now],
            )?;
            conn.last_insert_rowid()
        }
    };
    Ok(Note {
        id,
        body: input.body.clone(),
        pinned: input.pinned,
        updated_at: now,
    })
}

pub fn delete_note(conn: &Connection, id: i64) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM notes WHERE id = ?1", [id])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text_clip<'a>(text: &'a str, hash: &'a str) -> NewClip<'a> {
        NewClip {
            kind: "text",
            text: Some(text),
            image_path: None,
            thumb: None,
            width: 0,
            height: 0,
            hash,
        }
    }

    #[test]
    fn duplicate_copies_move_to_top_instead_of_repeating() {
        let db = Db::in_memory();
        db.with(|c| {
            upsert_clip(c, &text_clip("a", "h1"), 1)?;
            upsert_clip(c, &text_clip("b", "h2"), 2)?;
            upsert_clip(c, &text_clip("a", "h1"), 3)?;
            let list = list_clips(c, None, 10)?;
            assert_eq!(list.len(), 2);
            assert_eq!(list[0].text.as_deref(), Some("a"));
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn pinned_clips_stay_first_and_survive_clear() {
        let db = Db::in_memory();
        db.with(|c| {
            let (a, _) = upsert_clip(c, &text_clip("old", "h1"), 1)?;
            upsert_clip(c, &text_clip("new", "h2"), 2)?;
            set_clip_pinned(c, a.id, true)?;
            assert_eq!(list_clips(c, None, 10)?[0].text.as_deref(), Some("old"));
            clear_clips(c)?;
            let left = list_clips(c, None, 10)?;
            assert_eq!(left.len(), 1);
            assert!(left[0].pinned);
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn history_is_capped_but_pinned_items_are_kept() {
        let db = Db::in_memory();
        db.with(|c| {
            let (first, _) = upsert_clip(c, &text_clip("keep", "h0"), 0)?;
            set_clip_pinned(c, first.id, true)?;
            for i in 1..=(MAX_CLIPS + 5) {
                let (t, h) = (format!("t{i}"), format!("h{i}"));
                upsert_clip(c, &text_clip(&t, &h), i)?;
            }
            let all = list_clips(c, None, 1000)?;
            assert_eq!(all.len() as i64, MAX_CLIPS + 1);
            assert!(all.iter().any(|c| c.text.as_deref() == Some("keep")));
            assert!(!all.iter().any(|c| c.text.as_deref() == Some("t1")));
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn search_matches_text_and_escapes_wildcards() {
        let db = Db::in_memory();
        db.with(|c| {
            upsert_clip(c, &text_clip("hello world", "h1"), 1)?;
            upsert_clip(c, &text_clip("100% sure", "h2"), 2)?;
            assert_eq!(list_clips(c, Some("world"), 10)?.len(), 1);
            assert_eq!(list_clips(c, Some("%"), 10)?.len(), 1);
            assert_eq!(list_clips(c, Some("_"), 10)?.len(), 0);
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn notes_save_update_and_order() {
        let db = Db::in_memory();
        db.with(|c| {
            let a = save_note(
                c,
                &NoteInput {
                    id: None,
                    body: "first".into(),
                    pinned: false,
                },
                1,
            )?;
            let b = save_note(
                c,
                &NoteInput {
                    id: None,
                    body: "second".into(),
                    pinned: false,
                },
                2,
            )?;
            save_note(
                c,
                &NoteInput {
                    id: Some(a.id),
                    body: "first!".into(),
                    pinned: true,
                },
                3,
            )?;
            let list = list_notes(c)?;
            assert_eq!(list[0].body, "first!");
            assert_eq!(list[1].id, b.id);
            assert!(save_note(
                c,
                &NoteInput {
                    id: Some(999),
                    body: "x".into(),
                    pinned: false
                },
                4
            )
            .is_err());
            delete_note(c, b.id)?;
            assert_eq!(list_notes(c)?.len(), 1);
            Ok(())
        })
        .unwrap();
    }
}
