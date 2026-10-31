/**
 * Typed wrappers around the Rust commands and events.
 */

import { invoke } from "@tauri-apps/api/core";
import { listen, emit, type UnlistenFn } from "@tauri-apps/api/event";
import type { Rect } from "./shape";

export interface NowPlaying {
  title: string;
  artist: string;
  album: string;
  art: string | null;
  playing: boolean;
  positionMs: number;
  durationMs: number;
  source: string;
}

export type MediaAction = "play_pause" | "next" | "previous";

/** True when running inside the Tauri shell (false in a plain browser preview). */
export const inTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

async function safeInvoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T | null> {
  if (!inTauri) return null;
  try {
    return await invoke<T>(cmd, args);
  } catch (err) {
    console.warn(`[notcher] ${cmd} failed:`, err);
    return null;
  }
}

async function safeListen<T>(event: string, handler: (payload: T) => void): Promise<UnlistenFn> {
  if (!inTauri) return () => {};
  return listen<T>(event, (e) => handler(e.payload));
}

export const setHitRegion = (region: Rect) => safeInvoke("set_hit_region", { region });
export const focusIsland = () => safeInvoke("focus_island");
export const openSettings = () => safeInvoke("open_settings");
export const nowPlaying = () => safeInvoke<NowPlaying | null>("now_playing");
export const mediaCommand = (action: MediaAction) => safeInvoke("media_command", { action });
export const applyShortcut = async (shortcut: string): Promise<string | null> => {
  if (!inTauri) return null;
  try {
    await invoke("apply_shortcut", { shortcut });
    return null;
  } catch (err) {
    return String(err);
  }
};

export const onMedia = (h: (np: NowPlaying | null) => void) => safeListen("media://update", h);
export const onToggle = (h: () => void) => safeListen("island://toggle", h);
export const onSuppressed = (h: (v: { value: boolean }) => void) =>
  safeListen("island://suppressed", h);
export const onNativePointer = (h: (v: { value: boolean }) => void) =>
  safeListen("island://pointer", h);
export const onSettingsChanged = (h: () => void) => safeListen("settings://changed", h);
export const notifySettingsChanged = () => (inTauri ? emit("settings://changed") : undefined);

// ------------------------------------------------------------------ clipboard

export interface Clip {
  id: number;
  kind: "text" | "image";
  text: string | null;
  /** Thumbnail as a data URL (images only). */
  image: string | null;
  width: number;
  height: number;
  /** Unix milliseconds of the most recent copy. */
  createdAt: number;
  pinned: boolean;
}

export const clipsList = (query?: string) =>
  safeInvoke<Clip[]>("clips_list", { query: query?.trim() || null });
export const clipCopy = (id: number) => safeInvoke("clip_copy", { id });
export const clipSetPinned = (id: number, pinned: boolean) => safeInvoke("clip_set_pinned", { id, pinned });
export const clipDelete = (id: number) => safeInvoke("clip_delete", { id });
export const clipsClear = () => safeInvoke("clips_clear");
export const onClipAdded = (h: (clip: Clip) => void) => safeListen("clips://added", h);

// ------------------------------------------------------------------ notes

export interface Note {
  id: number;
  body: string;
  pinned: boolean;
  updatedAt: number;
}

export const notesList = () => safeInvoke<Note[]>("notes_list");
export const noteSave = (note: { id: number | null; body: string; pinned: boolean }) =>
  safeInvoke<Note>("note_save", { note });
export const noteDelete = (id: number) => safeInvoke("note_delete", { id });

// ------------------------------------------------------------------ system vitals

export interface Vitals {
  /** Overall CPU use, 0–100. */
  cpu: number;
  memUsed: number;
  memTotal: number;
  diskUsed: number;
  diskTotal: number;
  /** Network throughput in bytes per second. */
  netRx: number;
  netTx: number;
  battery: { percent: number; charging: boolean } | null;
  /** Last ~60 seconds, oldest first. */
  cpuHistory: number[];
  memHistory: number[];
}

export const vitalsSnapshot = () => safeInvoke<Vitals>("vitals_snapshot");
export const setVitalsActive = (active: boolean) => safeInvoke("set_vitals_active", { active });
export const onVitals = (h: (v: Vitals) => void) => safeListen("vitals://update", h);
