//! Notifications from other apps (WhatsApp, Telegram, Mail, …), shown in the island
//! as a short live pop-up.
//!
//! On Windows they are read from the notification centre with
//! `UserNotificationListener`. Apps installed without an MSIX package cannot
//! subscribe to its change event (it fails with "Element not found"), so the list
//! is polled twice a second instead, which is cheap and still feels instant.

use serde::Serialize;
use tauri::{AppHandle, Runtime};

#[cfg_attr(not(windows), allow(dead_code))]
pub const NOTIFY_EVENT: &str = "notify://new";

#[cfg_attr(not(windows), allow(dead_code))]
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AppNotification {
    /// The sending app, e.g. "WhatsApp".
    pub app: String,
    pub title: String,
    /// Empty when message previews are switched off.
    pub body: String,
    /// The app's logo as a `data:` URL, when Windows provides one.
    pub icon: Option<String>,
}

/// Turns a toast's text lines into a title and body. The first line is the title
/// (usually the sender); the rest is the message.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn split_text(app: &str, lines: &[String]) -> (String, String) {
    let mut lines = lines.iter().map(|l| l.trim()).filter(|l| !l.is_empty());
    let title = lines.next().unwrap_or(app).to_string();
    let body = lines.collect::<Vec<_>>().join(" · ");
    (title, body)
}

pub fn start<R: Runtime>(app: AppHandle<R>) {
    #[cfg(windows)]
    platform::spawn(app);
    #[cfg(not(windows))]
    let _ = app;
}

#[cfg(windows)]
mod platform {
    use super::{split_text, AppNotification, NOTIFY_EVENT};
    use crate::settings;
    use std::collections::HashSet;
    use std::time::Duration;
    use tauri::{AppHandle, Emitter, Runtime};
    use windows::{
        Foundation::Size,
        Storage::Streams::{DataReader, RandomAccessStreamReference},
        Win32::System::WinRT::{RoInitialize, RO_INIT_MULTITHREADED},
        UI::Notifications::{
            KnownNotificationBindings,
            Management::{UserNotificationListener, UserNotificationListenerAccessStatus},
            NotificationKinds, UserNotification,
        },
    };

    const POLL: Duration = Duration::from_millis(1000);

    pub fn spawn<R: Runtime>(app: AppHandle<R>) {
        let spawned = std::thread::Builder::new()
            .name("notcher-notifications".into())
            .spawn(move || run(app));
        if let Err(err) = spawned {
            log::error!("could not start notification reader: {err}");
        }
    }

    fn run<R: Runtime>(app: AppHandle<R>) {
        unsafe {
            let _ = RoInitialize(RO_INIT_MULTITHREADED);
        }
        let listener = match UserNotificationListener::Current() {
            Ok(l) => l,
            Err(err) => {
                log::warn!("notifications unavailable: {err}");
                return;
            }
        };
        match listener.RequestAccessAsync().and_then(|op| op.get()) {
            Ok(UserNotificationListenerAccessStatus::Allowed) => {}
            other => {
                log::warn!("notification access not granted ({other:?}); turn on Settings > Privacy > Notifications");
                return;
            }
        }
        // Notifications already in the centre at startup are old news.
        let mut seen = ids(&listener).unwrap_or_default();
        loop {
            std::thread::sleep(POLL);
            let Ok(list) = listener
                .GetNotificationsAsync(NotificationKinds::Toast)
                .and_then(|op| op.get())
            else {
                continue;
            };
            let mut current = HashSet::new();
            for n in &list {
                let Ok(id) = n.Id() else { continue };
                current.insert(id);
                if seen.contains(&id) || !settings::flag(&app, "notifications", true) {
                    continue;
                }
                if let Some(mut info) = describe(&n) {
                    if !settings::flag(&app, "notificationPreview", true) {
                        info.body.clear();
                    }
                    let _ = app.emit(NOTIFY_EVENT, &info);
                }
            }
            seen = current;
        }
    }

    fn ids(listener: &UserNotificationListener) -> windows::core::Result<HashSet<u32>> {
        let list = listener
            .GetNotificationsAsync(NotificationKinds::Toast)?
            .get()?;
        Ok((&list).into_iter().filter_map(|n| n.Id().ok()).collect())
    }

    fn describe(n: &UserNotification) -> Option<AppNotification> {
        let display = n.AppInfo().and_then(|a| a.DisplayInfo()).ok();
        let app = display
            .as_ref()
            .and_then(|d| d.DisplayName().ok())
            .map(|s| s.to_string_lossy())
            .unwrap_or_default();
        let binding = n
            .Notification()
            .ok()?
            .Visual()
            .ok()?
            .GetBinding(&KnownNotificationBindings::ToastGeneric().ok()?)
            .ok()?;
        let lines: Vec<String> = (&binding.GetTextElements().ok()?)
            .into_iter()
            .filter_map(|t| t.Text().ok())
            .map(|t| t.to_string_lossy())
            .collect();
        if lines.is_empty() && app.is_empty() {
            return None;
        }
        let (title, body) = split_text(&app, &lines);
        let icon = display
            .and_then(|d| {
                d.GetLogo(Size {
                    Width: 64.0,
                    Height: 64.0,
                })
                .ok()
            })
            .and_then(|logo| read_image(&logo).ok().flatten());
        Some(AppNotification {
            app,
            title,
            body,
            icon,
        })
    }

    fn read_image(r: &RandomAccessStreamReference) -> windows::core::Result<Option<String>> {
        let stream = r.OpenReadAsync()?.get()?;
        let size = stream.Size()? as u32;
        if size == 0 {
            return Ok(None);
        }
        let reader = DataReader::CreateDataReader(&stream.GetInputStreamAt(0)?)?;
        reader.LoadAsync(size)?.get()?;
        let mut bytes = vec![0u8; size as usize];
        reader.ReadBytes(&mut bytes)?;
        Ok(crate::media::data_url(&bytes))
    }
}

#[cfg(test)]
mod tests {
    use super::split_text;

    #[test]
    fn first_line_is_the_title_rest_is_the_body() {
        let lines = vec![
            "Mum".to_string(),
            "Dinner at 8?".to_string(),
            "".to_string(),
        ];
        assert_eq!(
            split_text("WhatsApp", &lines),
            ("Mum".to_string(), "Dinner at 8?".to_string())
        );
        assert_eq!(
            split_text("Telegram", &[]),
            ("Telegram".to_string(), String::new())
        );
    }
}
