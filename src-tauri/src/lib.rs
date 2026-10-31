mod beat;
mod clipboard;
mod db;
mod filedrop;
mod island;
mod media;
mod netspeed;
mod notes;
mod notifications;
mod platform;
mod screentime;
mod settings;
mod tray;
mod vitals;

use island::{HitRegion, IslandState, WINDOW_HEIGHT, WINDOW_WIDTH};
use platform::PlatformMode;
use tauri::{
    AppHandle, Emitter, Manager, Runtime, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

/// Asks the island to open (or close, if already open).
pub const TOGGLE_EVENT: &str = "island://toggle";

/// Hit region used until the web UI reports the real one: the resting island
/// plus a small margin, centred at the top of the window.
const STARTUP_REGION: HitRegion = HitRegion {
    x: (WINDOW_WIDTH - 264.0) / 2.0,
    y: 0.0,
    width: 264.0,
    height: 40.0,
};

#[tauri::command]
fn set_hit_region<R: Runtime>(
    window: WebviewWindow<R>,
    state: tauri::State<'_, IslandState>,
    region: HitRegion,
) {
    let region = region.clamped();
    if let Ok(mut current) = state.hit_region.lock() {
        if *current == region {
            return;
        }
        *current = region;
    }
    platform::apply_hit_region(&window, region);
}

/// Gives the island keyboard focus (used when it is opened with the shortcut).
#[tauri::command]
fn focus_island<R: Runtime>(window: WebviewWindow<R>) {
    let _ = window.set_focus();
}

/// Async on purpose: on Windows, creating a window from a synchronous command
/// deadlocks the event loop (the command runs on the thread that must build it).
#[tauri::command]
async fn open_settings<R: Runtime>(app: AppHandle<R>) {
    show_settings(&app);
}

/// Registers the open/close shortcut, replacing any previous one.
#[tauri::command]
fn apply_shortcut<R: Runtime>(app: AppHandle<R>, shortcut: String) -> Result<(), String> {
    register_shortcut(&app, &shortcut)
}

fn register_shortcut<R: Runtime>(app: &AppHandle<R>, shortcut: &str) -> Result<(), String> {
    // Check the text first, so a typo never removes the shortcut that works now.
    let parsed: Shortcut = shortcut.parse().map_err(|e| format!("{e}"))?;
    let manager = app.global_shortcut();
    if manager.is_registered(parsed) {
        return Ok(());
    }
    let _ = manager.unregister_all();
    if let Err(err) = manager.register(parsed) {
        // Usually another app owns that combination: keep the saved one working.
        let saved = settings::shortcut(app);
        if saved != shortcut {
            let _ = manager.register(saved.as_str());
        }
        return Err(err.to_string());
    }
    Ok(())
}

pub(crate) fn show_settings<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("settings") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
        return;
    }
    let built = WebviewWindowBuilder::new(app, "settings", WebviewUrl::App("settings.html".into()))
        .additional_browser_args(platform::WEBVIEW_ARGS)
        .title("Notcher Settings")
        .inner_size(560.0, 640.0)
        .min_inner_size(480.0, 520.0)
        .center()
        .build();
    if let Err(err) = built {
        log::error!("could not open settings: {err}");
    }
}

fn create_island<R: Runtime>(
    app: &AppHandle<R>,
) -> tauri::Result<(WebviewWindow<R>, PlatformMode, Option<island::MonitorRect>)> {
    let window = WebviewWindowBuilder::new(app, "island", WebviewUrl::App("index.html".into()))
        .additional_browser_args(platform::WEBVIEW_ARGS)
        .title("Notcher")
        .inner_size(WINDOW_WIDTH, WINDOW_HEIGHT)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .focused(false)
        .visible(false)
        .build()?;

    let mode = platform::prepare_island(&window);
    let mut monitor = None;
    if !mode.layer_shell {
        monitor = platform::place_on_best_monitor(&window);
        window.show()?;
    }
    // Applied once the window exists on screen. The shared state stays empty so the
    // web UI's first report is always applied too.
    platform::apply_hit_region(&window, STARTUP_REGION);
    Ok((window, mode, monitor))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    platform::before_gtk_init();

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            // Launching Notcher again opens its settings instead of a second island.
            show_settings(app);
        }))
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Info)
                .build(),
        )
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state == ShortcutState::Pressed {
                        let _ = app.emit(TOGGLE_EVENT, ());
                    }
                })
                .build(),
        )
        .on_window_event(|window, event| {
            // The island has no close button, but Alt+F4 could still close it; quitting
            // goes through the tray menu instead.
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "island" {
                    api.prevent_close();
                }
            }
        })
        .manage(IslandState::default())
        .invoke_handler(tauri::generate_handler![
            set_hit_region,
            focus_island,
            open_settings,
            apply_shortcut,
            media::media_command,
            media::now_playing,
            clipboard::clips_list,
            clipboard::clip_copy,
            clipboard::clip_set_pinned,
            clipboard::clip_delete,
            clipboard::clips_clear,
            notes::notes_list,
            notes::note_save,
            notes::note_delete,
            vitals::vitals_snapshot,
            vitals::set_vitals_active,
            beat::set_beat_active,
            screentime::screen_time,
            filedrop::drop_start,
            filedrop::drop_stop,
            filedrop::drop_state,
            filedrop::drop_share,
            filedrop::drop_unshare,
            filedrop::drop_thumb,
            filedrop::drop_clear_received,
            filedrop::drop_open,
        ])
        .setup(|app| {
            let handle = app.handle().clone();
            let data_dir = handle.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let db = db::Db::open(&data_dir.join("notcher.db"))
                .map_err(|e| format!("could not open the database: {e}"))?;
            handle.manage(db);
            let (_island, mode, monitor) = create_island(&handle)?;
            if let Err(err) = tray::create(&handle) {
                log::warn!("tray icon unavailable: {err}");
            }
            if let Err(err) = register_shortcut(&handle, &settings::shortcut(&handle)) {
                // Expected on some Wayland sessions; the tray and hover still work.
                log::warn!("global shortcut unavailable: {err}");
            }
            media::start(handle.clone());
            clipboard::start(handle.clone());
            vitals::start(handle.clone());
            notifications::start(handle.clone());
            beat::start(handle.clone());
            netspeed::start(handle.clone());
            screentime::start(handle.clone());
            platform::start_watcher(handle, mode, monitor);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Notcher");
}
