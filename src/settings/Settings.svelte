<script lang="ts">
  import { onMount } from "svelte";
  import { disable, enable, isEnabled } from "@tauri-apps/plugin-autostart";
  import { getVersion } from "@tauri-apps/api/app";
  import Toggle from "./Toggle.svelte";
  import { applyShortcut, inTauri, notifySettingsChanged } from "../lib/bridge";
  import {
    DEFAULT_SETTINGS,
    THEMES,
    applyTheme,
    loadSettings,
    saveSetting,
    type Settings,
  } from "../lib/settings";

  let settings = $state<Settings>({ ...DEFAULT_SETTINGS });
  let launchAtLogin = $state(false);
  let version = $state("");
  let shortcutDraft = $state(DEFAULT_SETTINGS.shortcut);
  let shortcutError = $state<string | null>(null);
  let shortcutSaved = $state(false);

  onMount(async () => {
    settings = await loadSettings();
    shortcutDraft = settings.shortcut;
    applyTheme(settings.theme);
    if (inTauri) {
      version = await getVersion().catch(() => "");
      launchAtLogin = await isEnabled().catch(() => false);
    }
  });

  async function update<K extends keyof Settings>(key: K, value: Settings[K]) {
    settings[key] = value;
    if (key === "theme") applyTheme(value as Settings["theme"]);
    if (!inTauri) return;
    await saveSetting(key, value);
    await notifySettingsChanged();
  }

  async function setLaunchAtLogin(value: boolean) {
    launchAtLogin = value;
    try {
      await (value ? enable() : disable());
    } catch (err) {
      console.warn("[notcher] autostart failed:", err);
      launchAtLogin = !value;
    }
  }

  async function saveShortcut() {
    const value = shortcutDraft.trim();
    shortcutSaved = false;
    shortcutError = await applyShortcut(value);
    if (!shortcutError) {
      await update("shortcut", value);
      shortcutSaved = true;
    }
  }
</script>

<main>
  <header>
    <div class="logo" aria-hidden="true"></div>
    <div>
      <h1>Notcher</h1>
      <p class="muted">Settings{version ? ` · v${version}` : ""}</p>
    </div>
  </header>

  <section>
    <h2>Appearance</h2>
    <div class="themes" role="radiogroup" aria-label="Theme">
      {#each THEMES as theme (theme.id)}
        <button
          class="theme"
          role="radio"
          aria-checked={settings.theme === theme.id}
          data-theme-preview={theme.id}
          onclick={() => update("theme", theme.id)}
        >
          <span class="swatch"><span class="pill"></span></span>
          {theme.label}
        </button>
      {/each}
    </div>
    <Toggle
      label="Glow"
      hint="A soft light around the island, tinted by the cover art"
      checked={settings.glow}
      onchange={(v) => update("glow", v)}
    />
    <Toggle
      label="Beat glow"
      hint="A soft light under the island that pulses with your music"
      checked={settings.beatGlow}
      onchange={(v) => update("beatGlow", v)}
    />
  </section>

  <section>
    <h2>Behaviour</h2>
    <Toggle
      label="Announce track changes"
      hint="Briefly expand the island when a new song starts"
      checked={settings.trackChangeActivity}
      onchange={(v) => update("trackChangeActivity", v)}
    />
    <Toggle
      label="Tuck away for maximized windows"
      hint="Shrink to a thin tab while a maximized or full-screen app is in front"
      checked={settings.tuckOnMaximized}
      onchange={(v) => update("tuckOnMaximized", v)}
    />
    <Toggle
      label="Follow the cursor"
      hint="Move the island to whichever screen you are using"
      checked={settings.followCursor}
      onchange={(v) => update("followCursor", v)}
    />
    <Toggle label="24-hour clock" hint="Off shows AM/PM" checked={settings.clock24h} onchange={(v) => update("clock24h", v)} />
  </section>

  <section>
    <h2>Notifications</h2>
    <Toggle
      label="Show notifications from other apps"
      hint="WhatsApp, Telegram, Mail and others pop out of the island. Needs Windows Settings › Privacy & security › Notifications › Allow apps to access your notifications."
      checked={settings.notifications}
      onchange={(v) => update("notifications", v)}
    />
    <Toggle
      label="Show message preview"
      hint="Off shows only the app and sender"
      checked={settings.notificationPreview}
      onchange={(v) => update("notificationPreview", v)}
    />
  </section>

  <section>
    <h2>Clipboard</h2>
    <Toggle
      label="Clipboard history"
      hint="Keep the last 200 things you copied. Passwords from password managers are skipped."
      checked={settings.clipboardHistory}
      onchange={(v) => update("clipboardHistory", v)}
    />
    <Toggle
      label="Copy confirmation"
      hint="Briefly show “Copied” in the island when you copy something"
      checked={settings.copyActivity}
      onchange={(v) => update("copyActivity", v)}
    />
  </section>

  <section>
    <h2>Shortcut</h2>
    <form class="shortcut" onsubmit={(e) => { e.preventDefault(); void saveShortcut(); }}>
      <input
        bind:value={shortcutDraft}
        spellcheck="false"
        aria-label="Open or close shortcut"
        placeholder="CommandOrControl+Alt+Space"
      />
      <button type="submit" class="primary">Apply</button>
    </form>
    {#if shortcutError}
      <p class="error">That shortcut could not be registered: {shortcutError}</p>
    {:else if shortcutSaved}
      <p class="ok">Saved.</p>
    {:else}
      <p class="muted small">Opens and closes the island from anywhere. Example: Ctrl+Alt+Space.</p>
    {/if}
  </section>

  <section>
    <h2>Startup</h2>
    <Toggle label="Launch at login" checked={launchAtLogin} onchange={setLaunchAtLogin} />
  </section>
</main>

<style>
  :global(body) {
    background: var(--window-bg);
    color: var(--fg);
  }
  main {
    max-width: 560px;
    margin: 0 auto;
    padding: 28px 28px 40px;
  }
  header {
    display: flex;
    align-items: center;
    gap: 14px;
    margin-bottom: 24px;
  }
  .logo {
    width: 44px;
    height: 44px;
    border-radius: 14px;
    background:
      radial-gradient(circle at 50% 0%, var(--accent) 0 28%, transparent 29%),
      var(--island-bg);
    box-shadow: inset 0 0 0 1px var(--hairline), 0 6px 18px var(--shadow);
  }
  h1 { font-size: 20px; }
  h2 {
    font-size: 12px;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--muted);
    margin-bottom: 6px;
  }
  section {
    background: var(--surface-1);
    border: 1px solid var(--hairline);
    border-radius: 16px;
    padding: 14px 18px;
    margin-bottom: 16px;
  }
  .muted { color: var(--muted); }
  .small { font-size: 12.5px; }
  .themes {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 10px;
    padding: 6px 0 12px;
  }
  .theme {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    padding: 8px;
    border: 1px solid var(--hairline);
    border-radius: 12px;
    background: transparent;
    color: var(--fg);
    font: inherit;
    font-size: 12.5px;
    cursor: pointer;
    transition: border-color 160ms, transform 160ms;
  }
  .theme:hover { transform: translateY(-1px); }
  .theme[aria-checked="true"] { border-color: var(--accent); box-shadow: 0 0 0 1px var(--accent); }
  .swatch {
    width: 100%;
    height: 40px;
    border-radius: 8px;
    display: flex;
    justify-content: center;
  }
  .pill { width: 50%; height: 12px; border-radius: 0 0 8px 8px; }
  [data-theme-preview="obsidian"] .swatch { background: #e9e9ef; }
  [data-theme-preview="obsidian"] .pill { background: #0b0b0d; }
  [data-theme-preview="daylight"] .swatch { background: #2a2a33; }
  [data-theme-preview="daylight"] .pill { background: #fbfbfc; }
  [data-theme-preview="neon"] .swatch { background: linear-gradient(135deg, #2b145c, #062b3a); }
  [data-theme-preview="neon"] .pill { background: #07060f; box-shadow: 0 0 8px #00f0c8; }
  [data-theme-preview="paper"] .swatch { background: #c9bba5; }
  [data-theme-preview="paper"] .pill { background: #f1ebe0; }
  .shortcut {
    display: flex;
    gap: 8px;
    padding: 8px 0;
  }
  .shortcut input {
    flex: 1;
    font: inherit;
    padding: 9px 12px;
    border-radius: 10px;
    border: 1px solid var(--hairline);
    background: var(--surface-1);
    color: var(--fg);
    user-select: text;
    -webkit-user-select: text;
  }
  .shortcut input:focus { outline: 2px solid var(--accent); outline-offset: -1px; }
  .primary {
    font: inherit;
    font-weight: 600;
    padding: 0 16px;
    border: 0;
    border-radius: 10px;
    background: var(--accent);
    color: #fff;
    cursor: pointer;
  }
  .error { color: #ff6b6b; font-size: 12.5px; }
  .ok { color: #3ccf8e; font-size: 12.5px; }
</style>
