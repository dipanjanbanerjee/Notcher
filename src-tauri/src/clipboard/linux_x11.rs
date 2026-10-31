//! X11 clipboard watcher.
//!
//! Uses the XFixes extension to be told when the clipboard changes (no polling), then
//! asks the new owner which formats it offers. Content flagged by password managers
//! (`x-kde-passwordManagerHint`) is skipped without being read.

use super::{capture, Content};
use crate::clipboard::image::Rgba;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Runtime};
use x11rb::connection::Connection;
use x11rb::protocol::xfixes::{ConnectionExt as _, SelectionEventMask};
use x11rb::protocol::xproto::{
    AtomEnum, ConnectionExt as _, CreateWindowAux, EventMask, PropMode, Property, WindowClass,
};
use x11rb::protocol::Event;
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;
use x11rb::CURRENT_TIME;

/// Largest transfer accepted (also caps INCR transfers).
const MAX_BYTES: usize = 64 * 1024 * 1024;
const TRANSFER_TIMEOUT: Duration = Duration::from_secs(2);

struct Atoms {
    clipboard: u32,
    targets: u32,
    utf8: u32,
    png: u32,
    incr: u32,
    property: u32,
    password_hint: u32,
}

struct Watcher {
    conn: RustConnection,
    window: u32,
    atoms: Atoms,
}

/// Starts the watcher thread. Returns false if X11 or XFixes is unavailable.
pub fn spawn<R: Runtime>(app: AppHandle<R>) -> bool {
    let Some(watcher) = Watcher::new() else {
        return false;
    };
    std::thread::Builder::new()
        .name("notcher-clipboard".into())
        .spawn(move || watcher.run(&app))
        .is_ok()
}

impl Watcher {
    fn new() -> Option<Self> {
        let (conn, screen) = x11rb::connect(None).ok()?;
        conn.xfixes_query_version(5, 0).ok()?.reply().ok()?;
        let root = conn.setup().roots.get(screen)?.root;
        let window = conn.generate_id().ok()?;
        conn.create_window(
            0,
            window,
            root,
            0,
            0,
            1,
            1,
            0,
            WindowClass::INPUT_ONLY,
            0,
            &CreateWindowAux::new().event_mask(EventMask::PROPERTY_CHANGE),
        )
        .ok()?;
        let atom = |name: &str| -> Option<u32> {
            Some(
                conn.intern_atom(false, name.as_bytes())
                    .ok()?
                    .reply()
                    .ok()?
                    .atom,
            )
        };
        let atoms = Atoms {
            clipboard: atom("CLIPBOARD")?,
            targets: atom("TARGETS")?,
            utf8: atom("UTF8_STRING")?,
            png: atom("image/png")?,
            incr: atom("INCR")?,
            property: atom("NOTCHER_CLIPBOARD")?,
            password_hint: atom("x-kde-passwordManagerHint")?,
        };
        conn.xfixes_select_selection_input(
            window,
            atoms.clipboard,
            SelectionEventMask::SET_SELECTION_OWNER,
        )
        .ok()?;
        conn.flush().ok()?;
        Some(Self {
            conn,
            window,
            atoms,
        })
    }

    fn run<R: Runtime>(self, app: &AppHandle<R>) {
        loop {
            let event = match self.conn.wait_for_event() {
                Ok(e) => e,
                Err(err) => {
                    log::warn!("clipboard watcher stopped: {err}");
                    return;
                }
            };
            if let Event::XfixesSelectionNotify(e) = event {
                if e.owner != x11rb::NONE && e.owner != self.window {
                    if let Some(content) = self.read() {
                        capture(app, content);
                    }
                }
            }
        }
    }

    fn read(&self) -> Option<Content> {
        let targets: Vec<u32> = {
            let bytes = self.convert(self.atoms.targets)?;
            bytes
                .chunks_exact(4)
                .map(|c| u32::from_ne_bytes([c[0], c[1], c[2], c[3]]))
                .collect()
        };
        if targets.contains(&self.atoms.password_hint) {
            return None;
        }
        if targets.contains(&self.atoms.utf8) {
            let bytes = self.convert(self.atoms.utf8)?;
            return Some(Content::Text(String::from_utf8_lossy(&bytes).into_owned()));
        }
        if targets.contains(&self.atoms.png) {
            let bytes = self.convert(self.atoms.png)?;
            return Rgba::from_png(&bytes).map(Content::Image);
        }
        None
    }

    /// Requests the clipboard in `target` format and waits for the owner's answer.
    fn convert(&self, target: u32) -> Option<Vec<u8>> {
        let a = &self.atoms;
        self.conn
            .convert_selection(self.window, a.clipboard, target, a.property, CURRENT_TIME)
            .ok()?;
        self.conn.flush().ok()?;
        let deadline = Instant::now() + TRANSFER_TIMEOUT;
        loop {
            match self.next_event(deadline)? {
                Event::SelectionNotify(e) if e.requestor == self.window => {
                    if e.property == x11rb::NONE {
                        return None;
                    }
                    break;
                }
                _ => continue,
            }
        }
        let reply = self
            .conn
            .get_property(
                true,
                self.window,
                a.property,
                AtomEnum::ANY,
                0,
                (MAX_BYTES / 4) as u32,
            )
            .ok()?
            .reply()
            .ok()?;
        if reply.type_ == a.incr {
            return self.read_incremental(deadline);
        }
        Some(reply.value)
    }

    /// Large transfers arrive in chunks, each announced by a property change.
    fn read_incremental(&self, deadline: Instant) -> Option<Vec<u8>> {
        let mut data = Vec::new();
        loop {
            match self.next_event(deadline)? {
                Event::PropertyNotify(e)
                    if e.window == self.window
                        && e.atom == self.atoms.property
                        && e.state == Property::NEW_VALUE =>
                {
                    let chunk = self
                        .conn
                        .get_property(
                            true,
                            self.window,
                            self.atoms.property,
                            AtomEnum::ANY,
                            0,
                            u32::MAX / 4,
                        )
                        .ok()?
                        .reply()
                        .ok()?;
                    if chunk.value.is_empty() {
                        return Some(data);
                    }
                    data.extend_from_slice(&chunk.value);
                    if data.len() > MAX_BYTES {
                        // Too big: abandon the transfer and clear our property.
                        let _ = self.conn.change_property8(
                            PropMode::REPLACE,
                            self.window,
                            self.atoms.property,
                            AtomEnum::STRING,
                            &[],
                        );
                        return None;
                    }
                }
                _ => continue,
            }
        }
    }

    fn next_event(&self, deadline: Instant) -> Option<Event> {
        loop {
            if let Some(event) = self.conn.poll_for_event().ok()? {
                return Some(event);
            }
            if Instant::now() >= deadline {
                return None;
            }
            std::thread::sleep(Duration::from_millis(4));
        }
    }
}
