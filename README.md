# Notcher

A fluid, living "island" at the top of your screen for **Windows 10 and 11**. It gives you music, clipboard history, quick notes, timers, a unit converter, weather and system stats, one hover away, and phone-to-PC file transfers by QR code.

**[Download for Windows](https://github.com/dipanjanbanerjee/notcher/releases/latest/download/Notcher-Setup.exe)** · [Website](https://dipanjanbanerjee.github.io/notcher/) · [All releases](https://github.com/dipanjanbanerjee/notcher/releases)

See the [product spec](docs/SPEC.md) for the full plan.

## Features

- **The island:** a notch-style shape that hangs from the top of the screen. At rest it shows the date and a live clock with seconds (12-hour with AM/PM, or 24-hour). Hover over it and it springs open into the full panel. Short *live* pop-ups, such as a new song or a finished timer, appear from rest and fold back by themselves. Every animation can be interrupted, and reduced-motion settings are respected.
- **Click-through:** only the island itself takes the mouse, and everything around it clicks straight through to your apps.
- **Now playing:** title, artist, cover art and progress from any media app (Spotify, browsers, VLC, Windows Media Player…), with play/pause, next and previous. A short *live* activity appears when the track changes.
- **Open panel with seven tabs** (`Ctrl+1` to `Ctrl+7`):
  - **Home:** clock, music controls, today's screen time, and quick looks at your timer, system, last copy and latest note. Press the pen button, then drag widgets to rearrange them, or remove and add them.
  - **Clipboard:** the last 200 things you copied, text and images, with search and pinning. Click to copy again. Passwords from password managers are never saved.
  - **Notes:** quick notes that save as you type, with `[ ]` checklists and pinning.
  - **Focus:** Pomodoro, countdown timer and stopwatch. The time stays visible on the resting island.
  - **Tools:** a unit converter (length, weight, temperature, volume, area, speed, data, time, and currency with daily exchange rates) next to the weather: now, feels-like, humidity, wind and the next five days. Click the place name to pick a city, or leave it blank to use your rough location. There's a Weather widget for Home too.
  - **System:** CPU and memory over the last minute, disk, battery, and today's screen time with the apps you used most.
  - **Drop:** Notcher Drop, see below.
- **Notcher Drop:** move files between your phone and PC by scanning a QR code. Your phone's browser opens a page where you can send photos and files to the PC (saved to `Downloads\Notcher Drop`) and save files the PC shares. Drag files onto the island to share them, even when it's tucked away. The Drop tab shows which phone is connected, live transfer speed and photo previews. It works over your own Wi-Fi with a one-time key, nothing goes through the internet, and it stops by itself after 10 idle minutes. The first time, Windows may ask to let Notcher through the firewall: allow it on private networks.
- **Glow:** a soft light around the island, tinted from the album art, plus a thin *beat glow* under its bottom edge that pulses with whatever is playing (read from the Windows volume meter; nothing is recorded).
- **Screen time:** how long you've been at your PC today and in which apps. Time away, a locked PC and the screen saver don't count; a full-screen video does. Only app names and totals are kept, on your PC.
- **Water reminder:** a gentle pop-up after every hour at the PC (or 30 min to 2 hours, or off, in Settings › Wellbeing). Time away doesn't count, and it waits until a full-screen video or game ends.
- **Notifications:** messages from WhatsApp, Telegram, Mail and other apps pop out of the island, with or without the message text. Windows asks once for notification access (Settings › Privacy & security › Notifications).
- **Steps aside for full-screen and maximized apps:** the island tucks into a thin tab at the top edge. Hover the tab to open it; song changes and notifications still pop out.
- **Internet speed on the taskbar:** upload and download speed, then how much data you've used today. In Settings › Taskbar you can pick small, medium or large text, put it on the left or right of the taskbar, nudge it with the arrow buttons if it covers other taskbar icons, or turn it off.
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

**Designing in a browser:** run `npm run dev` and open `http://localhost:1420/?demo`. You get a wallpaper and a fake track. Add `&mode=open` or `&mode=live` to jump to a state, and `&tab=clips` (or `home`, `jot`, `focus`, `tools`, `vitals`, `drop`) to pick a tab. In the console, `notcher.dispatch({type: "toggle"})` and `notcher.nextTrack()` drive the island.

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
  filedrop/          Notcher Drop: local server, QR code and the phone page
site/                the website (GitHub Pages)
```

### Releasing

Bump the version in `package.json`, `src-tauri/tauri.conf.json` and `src-tauri/Cargo.toml`, add notes in `docs/releases/vX.Y.Z.md`, and merge to `main`. The Release workflow sees a version with no release yet, builds the installers and publishes them under the tag `vX.Y.Z`. It can also be run by hand from the Actions tab. The website deploys from `site/` on every push to `main`.

### Troubleshooting

- **The island doesn't appear:** check that the WebView2 runtime is installed (Settings → Apps → "Microsoft Edge WebView2 Runtime"). The installer fetches it if it's missing.
- **Weather or currency says it can't connect:** they need the internet. Weather comes from Open-Meteo and rates from open.er-api.com; your rough location comes from your IP address unless you type a city.
- **Logs:** `%LOCALAPPDATA%\app.notcher.desktop\logs`.
- **Windows Defender or SmartScreen flags the installer:** Notcher isn't code-signed yet, and Defender's machine-learning check sometimes flags new unsigned apps (detections ending in `!ml`). Report it as a false positive at https://www.microsoft.com/en-us/wdsi/filesubmission.

## License

MIT © 2026 Dipanjan Banerjee
