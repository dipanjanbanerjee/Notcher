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
