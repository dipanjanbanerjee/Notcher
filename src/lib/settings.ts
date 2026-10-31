/**
 * User settings, persisted with tauri-plugin-store in `settings.json`.
 */

import { load, type Store } from "@tauri-apps/plugin-store";

export type ThemeName = "obsidian" | "daylight" | "neon" | "paper";

export interface Settings {
  theme: ThemeName;
  /** Ambient light around the island, tinted from cover art. */
  glow: boolean;
  /** Show a live activity when the track changes. */
  trackChangeActivity: boolean;
  /** Move the island to the monitor the cursor is on. */
  followCursor: boolean;
  shortcut: string;
  /** Use 24-hour time (otherwise 12-hour with AM/PM). */
  clock24h: boolean;
  /** Keep a history of copied text and images. */
  clipboardHistory: boolean;
  /** Briefly show "Copied" in the island when something is copied. */
  copyActivity: boolean;
  /** Pop up notifications from other apps (WhatsApp, Telegram, …). */
  notifications: boolean;
  /** Include the message text in those pop-ups. */
  notificationPreview: boolean;
  /** Tuck the island away while a maximized window is in front, as for full screen. */
  tuckOnMaximized: boolean;
  /** A soft light under the island that moves with the music. */
  beatGlow: boolean;
  /** Internet speed meter on the taskbar, next to the tray icons. */
  taskbarSpeed: boolean;
  /** Text size of the taskbar meter. */
  taskbarSpeedSize: "small" | "medium" | "large";
  /** Which end of the taskbar the meter sits on. */
  taskbarSpeedPosition: "left" | "right";
  /** Show how much data was used today next to the speeds. */
  taskbarSpeedToday: boolean;
  /** Minutes of PC use between "drink some water" pop-ups; 0 turns them off. */
  waterReminder: number;
}

/** Choices for the water reminder, in minutes (0 = off). */
export const WATER_CHOICES = [0, 30, 45, 60, 90, 120] as const;

export const DEFAULT_SETTINGS: Settings = {
  theme: "obsidian",
  glow: true,
  trackChangeActivity: true,
  followCursor: true,
  shortcut: "CommandOrControl+Alt+Space",
  clock24h: false,
  clipboardHistory: true,
  copyActivity: true,
  notifications: true,
  notificationPreview: true,
  tuckOnMaximized: true,
  beatGlow: true,
  taskbarSpeed: true,
  taskbarSpeedSize: "medium",
  taskbarSpeedPosition: "right",
  taskbarSpeedToday: true,
  waterReminder: 60,
};

export const THEMES: { id: ThemeName; label: string }[] = [
  { id: "obsidian", label: "Obsidian" },
  { id: "daylight", label: "Daylight" },
  { id: "neon", label: "Neon" },
  { id: "paper", label: "Paper" },
];

export const SETTINGS_CHANGED = "settings://changed";

/** Keeps only known keys with the right types, filling the rest with defaults. */
export function sanitize(raw: Record<string, unknown>): Settings {
  const out = { ...DEFAULT_SETTINGS };
  for (const key of Object.keys(DEFAULT_SETTINGS) as (keyof Settings)[]) {
    const value = raw[key];
    if (typeof value === typeof DEFAULT_SETTINGS[key]) {
      (out as Record<string, unknown>)[key] = value;
    }
  }
  if (!THEMES.some((t) => t.id === out.theme)) out.theme = DEFAULT_SETTINGS.theme;
  if (!["small", "medium", "large"].includes(out.taskbarSpeedSize)) out.taskbarSpeedSize = "medium";
  if (out.taskbarSpeedPosition !== "left") out.taskbarSpeedPosition = "right";
  if (!(WATER_CHOICES as readonly number[]).includes(out.waterReminder)) out.waterReminder = DEFAULT_SETTINGS.waterReminder;
  return out;
}

let storePromise: Promise<Store> | null = null;
function store(): Promise<Store> {
  storePromise ??= load("settings.json", { defaults: {}, autoSave: 200 });
  return storePromise;
}

export async function loadSettings(): Promise<Settings> {
  try {
    const s = await store();
    return sanitize(Object.fromEntries(await s.entries()));
  } catch {
    return { ...DEFAULT_SETTINGS };
  }
}

export async function saveSetting<K extends keyof Settings>(key: K, value: Settings[K]) {
  const s = await store();
  await s.set(key, value);
  await s.save();
}

export function applyTheme(theme: ThemeName) {
  document.documentElement.dataset.theme = theme;
}
