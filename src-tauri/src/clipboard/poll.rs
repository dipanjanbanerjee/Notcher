//! Clipboard watcher for Windows and native Wayland sessions, built on `arboard`.
//!
//! On Windows the clipboard sequence number tells us cheaply when something changed,
//! and apps can mark content as private (password managers do), which is respected.
//! Elsewhere the clipboard is compared by hash once a second.

use super::{capture, image::fnv1a, image::Rgba, Content};
use std::time::Duration;
use tauri::{AppHandle, Runtime};

pub fn spawn<R: Runtime>(app: AppHandle<R>) {
    let spawned = std::thread::Builder::new()
        .name("notcher-clipboard".into())
        .spawn(move || {
            let mut clipboard = match arboard::Clipboard::new() {
                Ok(c) => c,
                Err(err) => {
                    log::warn!("clipboard history unavailable: {err}");
                    return;
                }
            };
            let mut change = ChangeDetector::default();
            // Do not import whatever was on the clipboard before Notcher started.
            let mut last_hash = read(&mut clipboard).map(|c| hash(&c));
            loop {
                std::thread::sleep(POLL);
                if !change.changed() {
                    continue;
                }
                if platform::is_private() {
                    change.done();
                    continue;
                }
                let Some(content) = read(&mut clipboard) else {
                    // The app that just copied may still hold the clipboard open, so a
                    // failed read is tried again on the next few polls.
                    change.failed();
                    continue;
                };
                change.done();
                let h = hash(&content);
                if last_hash.as_deref() != Some(h.as_str()) {
                    last_hash = Some(h);
                    capture(&app, content);
                }
            }
        });
    if let Err(err) = spawned {
        log::error!("could not start clipboard watcher: {err}");
    }
}

#[cfg(windows)]
const POLL: Duration = Duration::from_millis(300);
#[cfg(not(windows))]
const POLL: Duration = Duration::from_millis(1000);

fn read(cb: &mut arboard::Clipboard) -> Option<Content> {
    if let Ok(text) = cb.get_text() {
        return Some(Content::Text(text));
    }
    let img = cb.get_image().ok()?;
    Rgba::new(img.width as u32, img.height as u32, img.bytes.into_owned()).map(Content::Image)
}

fn hash(content: &Content) -> String {
    match content {
        Content::Text(t) => fnv1a(&[b"text", t.as_bytes()]),
        Content::Image(i) => fnv1a(&[b"image", &i.pixels]),
    }
}

/// Retries for a change whose content could not be read yet (about a second).
#[cfg_attr(not(windows), allow(dead_code))]
const READ_RETRIES: u8 = 3;

#[derive(Default)]
struct ChangeDetector {
    #[cfg(windows)]
    last: u32,
    /// A change still waiting to be read, with the number of failed reads so far.
    #[cfg(windows)]
    pending: Option<u8>,
}

impl ChangeDetector {
    #[cfg(windows)]
    fn changed(&mut self) -> bool {
        let seq = unsafe { windows::Win32::System::DataExchange::GetClipboardSequenceNumber() };
        if seq != self.last {
            self.last = seq;
            self.pending = Some(0);
        }
        self.pending.is_some()
    }

    /// The latest change was handled (or is not ours to read).
    #[cfg(windows)]
    fn done(&mut self) {
        self.pending = None;
    }

    /// Reading failed; keep the change for a few more polls, then give up (files and
    /// other formats Notcher does not keep can never be read).
    #[cfg(windows)]
    fn failed(&mut self) {
        self.pending = match self.pending {
            Some(tries) if tries + 1 < READ_RETRIES => Some(tries + 1),
            _ => None,
        };
    }

    /// Without a cheap change signal, every poll reads and compares.
    #[cfg(not(windows))]
    fn changed(&mut self) -> bool {
        true
    }
    #[cfg(not(windows))]
    fn done(&mut self) {}
    #[cfg(not(windows))]
    fn failed(&mut self) {}
}

#[cfg(windows)]
mod platform {
    use windows::core::w;
    use windows::Win32::System::DataExchange::{
        IsClipboardFormatAvailable, RegisterClipboardFormatW,
    };

    /// Formats that apps set to keep content out of clipboard history and monitors.
    pub fn is_private() -> bool {
        unsafe {
            [
                RegisterClipboardFormatW(w!("ExcludeClipboardContentFromMonitorProcessing")),
                RegisterClipboardFormatW(w!("Clipboard Viewer Ignore")),
            ]
            .into_iter()
            .any(|f| f != 0 && IsClipboardFormatAvailable(f).is_ok())
        }
    }
}

#[cfg(not(windows))]
mod platform {
    pub fn is_private() -> bool {
        false
    }
}
