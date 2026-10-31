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
  /** Fine-tuning of the meter's place on the taskbar, in pixels (negative is left). */
  taskbarSpeedOffset: number;
  /** Minutes of PC use between "drink some water" pop-ups; 0 turns them off. */
  waterReminder: number;
  /** Name of the place picked for the weather; empty means "find me automatically". */
  weatherCity: string;
  /** Where that place is (0, 0 when not picked). */
  weatherLat: number;
  weatherLon: number;
  /** Celsius or Fahrenheit for the weather. */
  tempUnit: "c" | "f";
  /** Check for a new version at startup and install it. */
  autoUpdate: boolean;
}

/** How far the taskbar meter can be nudged either way, in pixels. */
export const MAX_METER_OFFSET = 600;

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
  taskbarSpeedOffset: 0,
  waterReminder: 60,
  weatherCity: "",
  weatherLat: 0,
  weatherLon: 0,
  tempUnit: "c",
  autoUpdate: true,
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
  out.taskbarSpeedOffset = Number.isFinite(out.taskbarSpeedOffset)
    ? Math.max(-MAX_METER_OFFSET, Math.min(MAX_METER_OFFSET, Math.round(out.taskbarSpeedOffset)))
    : 0;
  out.weatherCity = out.weatherCity.trim().slice(0, 80);
  const validCoord = (v: number, limit: number) => Number.isFinite(v) && Math.abs(v) <= limit;
  if (!validCoord(out.weatherLat, 90) || !validCoord(out.weatherLon, 180)) {
    out.weatherLat = 0;
    out.weatherLon = 0;
  }
  if (out.tempUnit !== "f") out.tempUnit = "c";
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
