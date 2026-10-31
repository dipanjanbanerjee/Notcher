# Notcher — Product Spec (v0.1 draft)

Notcher is a desktop "island" for **Windows and Linux**. It is a small, living shape at the top of the screen that expands into a panel of everyday tools when you need it and steps out of the way when you don't.

This document defines what we build, how it should feel, and the order we build it in.

---

## 1. Goals

1. **Feel premium.** Every transition should be physically believable, using spring-based motion, interruptible animations and no jank. Target 60 fps or higher, and hold 120 fps on high-refresh displays.
2. **Cross-platform from day one.** Windows 10 (1809+) and 11, and Linux (KDE, Hyprland/Sway and other wlroots compositors, X11 desktops, and GNOME with documented limits).
3. **Reliable.** No stuck windows, no focus stealing, no CPU spikes, and no lost data. Crashes are caught and reported locally.
4. **Private by default.** Everything stays on the machine. Network is used only for features that obviously need it, such as weather, currency, calendar subscriptions and the integrations the user connects.
5. **Our own identity.** Notcher has its own visual language, names, mascot and copy. See [§9 Originality rules](#9-originality-rules).

## 2. Platforms & tech stack

| Layer | Choice | Why |
|---|---|---|
| App shell | **Tauri 2** (Rust) | Small binaries (~10 MB), low RAM, one codebase for Windows and Linux |
| UI | **Svelte 5 + TypeScript** | Fast, tiny runtime, and good for animation-heavy UI |
| Motion | Custom spring engine (Rust-driven window resize + CSS/JS springs for content) | Fluid morphing of the island shape |
| Storage | SQLite (via `rusqlite`) in the user data dir | Clipboard history, notes, screen time, settings |
| Packaging | Windows: MSIX/NSIS. Linux: AppImage, .deb, .rpm, Flatpak (later) | |

### Platform adapters (Rust, behind one trait per capability)

| Capability | Windows | Linux |
|---|---|---|
| Top-anchored overlay window | Win32 layered, topmost, tool window, no taskbar entry | Wayland: `wlr-layer-shell` (KDE, Hyprland, Sway). X11: override-redirect / dock hint. GNOME Wayland: fallback (see §8) |
| Now playing + media control | `GlobalSystemMediaTransportControlsSessionManager` (SMTC) | MPRIS over D-Bus |
| Volume | Core Audio (`IAudioEndpointVolume`) | PipeWire / PulseAudio |
| Clipboard watch | `AddClipboardFormatListener` | Wayland `wlr-data-control` / `ext-data-control`; X11 XFixes |
| Active app (screen time) | `GetForegroundWindow` + process name | X11 `_NET_ACTIVE_WINDOW`; KDE/wlroots via compositor protocols; GNOME via extension (later) |
| Full-screen detection | `SHQueryUserNotificationState` + monitor rect check | Layer-shell/EWMH full-screen state |
| System stats | `sysinfo` crate | `sysinfo` crate |
| Global shortcut | Tauri global-shortcut plugin | Same (X11), XDG GlobalShortcuts portal (Wayland) |

## 3. The island (core interaction)

The island is the heart of the product, and the main place where we beat existing apps.

**States**
1. **Rest:** a slim pill at the top-center of the active monitor. Optional "ambient" content can sit inside it, such as an album-art dot or a timer countdown.
2. **Peek:** on hover, the pill grows slightly and shows a one-line summary. It opens after a 120 ms intent delay so passing the cursor over it doesn't trigger it.
3. **Open:** on click, a shortcut (default `Ctrl+Alt+Space`) or a hover dwell, the island morphs into the full panel with tool tabs.
4. **Live activity:** a temporary expanded state for events such as a track change, volume change, timer finished, copy captured or file received. It collapses automatically.

**Motion principles**
- Spring physics, roughly stiffness 380 and damping 32, tuned per transition. Never use linear or fixed-duration easing for the shape.
- Every animation can be interrupted. If the user moves away mid-open, the island reverses from its current position without snapping.
- Content cross-fades with a slight blur and scale (0.96 to 1) *after* the shape has passed 60% of its target size, so text never squashes.
- Corner radius changes continuously with height, so the shape stays organic.
- Respect "reduce motion" OS settings by switching to short fades.

**Behaviour**
- Multi-monitor: follow the cursor's monitor, or pin to one monitor (setting).
- Hides automatically for full-screen games and video, presentations, and optionally for screen sharing.
- Never steals focus when showing live activities.
- Drag a file onto the island from anywhere to add it to the Tray (see §4).

## 4. Tools

Every tool is a module. Users enable, disable and reorder them in Settings. Names below are Notcher's own.

### 4.1 Feature-parity set

| # | Notcher tool | What it does |
|---|---|---|
| 1 | **Glance** | Home dashboard: now playing, next event, weather, active timer, last clip, today's screen time |
| 2 | **Vitals** | CPU, RAM, disk and GPU usage with 60-second sparklines, network up/down, battery, Wi-Fi, volume slider |
| 3 | **Clocks** | Local time and world clocks, with a "best meeting time" overlap bar across selected zones |
| 4 | **Focus** | Pomodoro, countdown, stopwatch and a hydration nudge. The time left shows in the island at rest |
| 5 | **Agenda** | Local events and reminders, ICS subscriptions, regional public holidays, and a countdown to the next event |
| 6 | **Sky** | Current weather, hourly and 7-day forecasts, humidity, wind and UV, with °C/°F toggle |
| 7 | **Clips** | Clipboard history (text, links, images), search, pins, drag-out. Skips password-manager content and "sensitive" clipboard flags |
| 8 | **Jot** | Quick notes and checklists with autosave, pinnable to the island at rest |
| 9 | **Tray** | Drop files on the island to park them, with multiple trays, drag-out, "copy all" and "zip all" |
| 10 | **Beam** | Phone ↔ PC transfer over the local network. Scan a QR code and use the phone's browser, no app needed. Optional PIN, TLS with per-session keys |
| 11 | **Morph** | Image converter: JPG, PNG, WebP, PDF, ICO, plus compress-to-target-size, all offline |
| 12 | **Convert** | Unit and currency conversion with history |
| 13 | **Launch** | Pinned sites, search shortcuts (web, YouTube, GitHub, Stack Overflow and custom ones), and "save copied link" |
| 14 | **Banner** | A full-width status message across the top of the screen ("In a meeting", "On call"), with color and motion presets |
| 15 | **Usage** | Per-app screen time for today and the week, with optional daily limits and gentle nudges |
| 16 | **Pulse** | Site analytics from Plausible / DataFast (visitors, pages, sources, countries, live count) |
| 17 | **Till** | Revenue from Stripe / Gumroad (today's net, balance), with a live activity on each new sale |
| 18 | **Tokens** | AI coding usage from local Claude Code / Codex logs: tokens, sessions, estimated cost, streak |

### 4.2 Notcher-only features (our edge)

- **Command bar:** type straight into the open island to run any action ("timer 25", "convert 5 km to mi", "note buy milk", "beam"). It is keyboard-first.
- **Glow:** an ambient light that traces the island's edge, colored from album art or an accent color. It pulses gently for notifications, and the user chooses the effect style.
- **Mote (mascot):** a tiny, abstract light-creature that lives in the island. It reacts to music beats, dozes when you're idle, cheers when a timer finishes, and is never cutesy unless you want it to be. It is fully optional.
- **Profiles:** different tool sets and layouts for "Work", "Gaming" and "Present", which can switch automatically by app or time.
- **Plugin API (later):** third-party modules in a sandboxed WebView with a small typed API.
- **Linux first-class:** KDE Connect bridge for Beam, and a GNOME Shell extension companion.

## 5. Visual design

- **Shape:** a true superellipse ("squircle") rather than a plain rounded rectangle, with a 1 px inner highlight and a soft outer shadow that grows with elevation.
- **Material:** deep graphite with a subtle grain. Optional translucency uses Mica/Acrylic on Windows 11 and blur on KDE where available.
- **Themes (initial):** *Obsidian* (default dark), *Daylight* (light), *Neon* (high-saturation accents), *Paper* (warm, low-contrast).
- **Type:** Inter (UI) + JetBrains Mono (numbers and timers), with tabular numerals everywhere numbers change.
- **Icons:** Lucide-based, custom-tuned. No icons or artwork are taken from other notch apps.

## 6. Settings

General (start on login, shortcut, monitor, hover sensitivity), Appearance (theme, island size, Glow, Mote), Tools (enable, reorder, per-tool settings), Integrations (API keys stored in the OS keychain: Windows Credential Manager or Secret Service on Linux), Privacy (clipboard exclusions, history retention, clear data), and About and updates.

## 7. Privacy & security

- No account and no telemetry. Crash logs are written locally, and the user can choose to share one.
- API tokens are kept in the OS keychain, never in plain files.
- The Beam server only runs while the Beam panel is open, binds to the LAN interface, uses a random port, a one-time token in the QR code, an optional PIN and TLS.
- Clipboard history honours `ExcludeClipboardContentFromMonitorProcessing` (Windows) and `x-kde-passwordManagerHint` (Linux).

## 8. Linux support matrix

| Desktop | Island | Media | Clipboard history | Screen time |
|---|---|---|---|---|
| KDE Plasma 6 (Wayland) | ✅ layer-shell | ✅ | ✅ | ✅ |
| Hyprland / Sway | ✅ layer-shell | ✅ | ✅ | ✅ |
| X11 (any) | ✅ | ✅ | ✅ | ✅ |
| GNOME (Wayland) | ⚠️ regular always-on-top window. Full support needs our GNOME Shell extension | ✅ | ⚠️ needs extension | ⚠️ needs extension |

## 9. Originality rules

We match *capabilities*, which nobody owns. We never copy *expression*.

- No code, assets, icons, screenshots, text, mascot designs, theme names or tool names from Vayli or any other notch app.
- Notcher's own names for everything (see §4), its own visual identity (§5) and its own mascot (Mote).
- Website and store copy are written from scratch.
- When in doubt, design from the user's problem rather than from someone else's screen.

## 10. Milestones

| Milestone | Scope |
|---|---|
| **M1 — The island** | Tauri app, top-anchored island on Windows + KDE/wlroots/X11, rest/peek/open/live states, spring engine, multi-monitor, full-screen hiding, tray icon, settings shell |
| **M2 — Daily drivers** | Now playing (SMTC/MPRIS) with Glow, volume live activity, Clips, Jot, Focus, Vitals, Glance |
| **M3 — Files** | Tray, Beam, Morph |
| **M4 — Info** | Clocks, Agenda, Sky, Convert, Launch, Banner, Usage |
| **M5 — Maker tools** | Pulse, Till, Tokens, keychain-backed integrations |
| **M6 — Delight & ship** | Command bar, Mote, Profiles, themes, installers, auto-update, website |

## 11. Open questions

1. Is the app name **Notcher** final?
2. Business model: one-time license, free + Pro, or open source?
3. Which Linux desktops matter most to you for M1?
