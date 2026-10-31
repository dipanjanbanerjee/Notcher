# Notcher

A fluid, living "island" at the top of your screen for **Windows and Linux**. It gives you music, clipboard history, quick notes, timers, file transfers and more, one hover away.

> Status: early development (milestone 1). See the [product spec](docs/SPEC.md) for the full plan.

## What works today

- **The island:** a notch-style shape that hangs from the top of the screen and morphs between *rest → peek → live → open* with spring physics. Every animation can be interrupted, and reduced-motion settings are respected.
- **Click-through:** only the island itself takes the mouse, and everything around it clicks straight through to your apps.
- **Now playing:** title, artist, cover art and progress from any media app (Spotify, browsers, VLC, Windows Media Player…), with play/pause, next and previous. A short *live* activity appears when the track changes.
- **Glow:** a soft light around the island, tinted from the album art.
- **Steps aside for full-screen apps:** games, videos and presentations.
- **Multi-monitor:** the island follows the screen your cursor is on.
- **Shortcut:** `Ctrl+Alt+Space` opens and closes the island from anywhere (you can change it).
- **Themes:** Obsidian, Daylight, Neon, Paper.
- Tray icon, settings window, launch at login, single instance.

## Platform support

| Platform | Island | Click-through | Media | Full-screen hiding |
|---|---|---|---|---|
| Windows 10 (1809+) / 11 | ✅ | ✅ | ✅ (SMTC) | ✅ |
| Linux X11 (GNOME, KDE, XFCE, …) | ✅ | ✅ | ✅ (MPRIS) | ✅ |
| Linux Wayland: KDE, Hyprland, Sway | ✅ layer-shell | ✅ | ✅ | ✅ (by the compositor) |
| Linux Wayland: GNOME | ✅ via XWayland | ✅ | ✅ | ⚠️ X11 apps only |

The global shortcut uses X11 key grabs, so on Wayland it may be unavailable. The tray menu and hover still work there.

## Development

Requirements: Node 22+, Rust (stable), and the Tauri prerequisites for your OS.

On Ubuntu/Debian, install the system libraries:

```sh
sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev libgtk-layer-shell-dev \
  librsvg2-dev libayatana-appindicator3-dev libdbus-1-dev libxdo-dev
```

Windows needs the Microsoft C++ Build Tools and WebView2, which is preinstalled on Windows 11.

```sh
npm install
npm run tauri dev        # run the app with hot reload
npm test                 # UI unit tests
npm run check            # type-check
cd src-tauri && cargo test
npm run tauri build      # installers in src-tauri/target/release/bundle
```

**Designing in a browser:** run `npm run dev` and open `http://localhost:1420/?demo`. You get a wallpaper and a fake track. Add `&mode=open`, `&mode=peek` or `&mode=live` to jump to a state. In the console, `notcher.dispatch({type: "toggle"})` and `notcher.nextTrack()` drive the island.

### Project layout

```
src/                 Svelte + TypeScript UI
  island/            the island window
  settings/          the settings window
  lib/               springs, shape geometry, state machine, colour, settings
src-tauri/src/       Rust
  island.rs          window geometry and hit regions
  platform/          Windows & Linux window behaviour (click-through, layer-shell, full-screen)
  media/             now playing: SMTC (Windows) and MPRIS (Linux)
```

### Troubleshooting

- **Black box instead of a transparent island on Linux:** your session needs a compositor. GNOME, KDE and most desktops have one. On bare X11 window managers, run `picom`.
- **Force a backend:** `NOTCHER_BACKEND=x11 notcher` or `NOTCHER_BACKEND=wayland notcher`.

## License

MIT © 2026 Dipanjan Banerjee
