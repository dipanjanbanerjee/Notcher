# Notcher

A fluid, living "island" at the top of your screen for **Windows 10 and 11**. It gives you music, clipboard history, quick notes, timers and system stats, one hover away, with file transfers and more on the way.

> Status: early development (milestone 2). See the [product spec](docs/SPEC.md) for the full plan.

## What works today

- **The island:** a notch-style shape that hangs from the top of the screen. At rest it shows the date and a live clock with seconds (12-hour with AM/PM, or 24-hour). Hover over it and it springs open into the full panel. Short *live* pop-ups, such as a new song or a finished timer, appear from rest and fold back by themselves. Every animation can be interrupted, and reduced-motion settings are respected.
- **Click-through:** only the island itself takes the mouse, and everything around it clicks straight through to your apps.
- **Now playing:** title, artist, cover art and progress from any media app (Spotify, browsers, VLC, Windows Media Player…), with play/pause, next and previous. A short *live* activity appears when the track changes.
- **Open panel with five tabs** (`Ctrl+1` to `Ctrl+5`):
  - **Home:** clock, music controls, and quick looks at your timer, system, last copy and latest note. Press the pen button to move, remove or add widgets.
  - **Clipboard:** the last 200 things you copied, text and images, with search and pinning. Click to copy again. Passwords from password managers are never saved.
  - **Notes:** quick notes that save as you type, with `[ ]` checklists and pinning.
  - **Focus:** Pomodoro, countdown timer and stopwatch. The time stays visible on the resting island, and a water reminder can nudge you.
  - **System:** CPU and memory over the last minute, disk and battery.
- **Glow:** a soft light around the island, tinted from the album art, plus a thin *beat glow* under its bottom edge that pulses with whatever is playing (read from the Windows volume meter; nothing is recorded).
- **Notifications:** messages from WhatsApp, Telegram, Mail and other apps pop out of the island, with or without the message text. Windows asks once for notification access (Settings › Privacy & security › Notifications).
- **Steps aside for full-screen and maximized apps:** the island tucks into a thin tab at the top edge. Hover the tab to open it; song changes and notifications still pop out.
- **Internet speed on the taskbar:** upload and download speed sit next to the tray icons. Turn it off in Settings › Taskbar.
- **Multi-monitor:** the island follows the screen your cursor is on.
- **Shortcut:** `Ctrl+Alt+Space` opens and closes the island from anywhere (you can change it).
- **Themes:** Obsidian, Daylight, Neon, Paper.
- Tray icon, settings window, launch at login, single instance.

## Platform support

Notcher is built for **Windows 10 (1809 or later) and Windows 11**. It uses Windows' own media controls (SMTC) for music, so any player that shows up in the Windows volume flyout works.

## Development

Requirements: Node 22+, Rust (stable), the Microsoft C++ Build Tools, and WebView2 (preinstalled on Windows 11).

```sh
npm install
npm run tauri dev        # run the app with hot reload
npm test                 # UI unit tests
npm run check            # type-check
cd src-tauri && cargo test
npm run tauri build      # installers in src-tauri/target/release/bundle (nsis, msi)
```

**Designing in a browser:** run `npm run dev` and open `http://localhost:1420/?demo`. You get a wallpaper and a fake track. Add `&mode=open` or `&mode=live` to jump to a state, and `&tab=clips` (or `home`, `jot`, `focus`, `vitals`) to pick a tab. In the console, `notcher.dispatch({type: "toggle"})` and `notcher.nextTrack()` drive the island.

### Project layout

```
src/                 Svelte + TypeScript UI
  island/            the island window (panel/ holds the open-panel tabs)
  settings/          the settings window
  lib/               springs, shape geometry, state machine, timers, settings, stores
src-tauri/src/       Rust
  island.rs          window geometry and hit regions
  platform/          window behaviour (click-through, full-screen hiding, monitors)
  media/             now playing through SMTC
  clipboard/         clipboard history
  db.rs              SQLite storage for clips and notes
  vitals.rs          CPU, memory, disk, network and battery
  netspeed.rs        the internet speed meter on the taskbar
```

### Troubleshooting

- **The island doesn't appear:** check that the WebView2 runtime is installed (Settings → Apps → "Microsoft Edge WebView2 Runtime"). The installer fetches it if it's missing.
- **Logs:** `%LOCALAPPDATA%\app.notcher.desktop\logs`.

## License

MIT © 2026 Dipanjan Banerjee
