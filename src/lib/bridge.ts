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

// ------------------------------------------------------------------ notifications & beat

export interface AppNotification {
  app: string;
  title: string;
  body: string;
  icon: string | null;
}

export const onAppNotification = (h: (n: AppNotification) => void) => safeListen("notify://new", h);

/** Speaker level, 0..1, while the beat glow is active. */
export const onAudioLevel = (h: (level: number) => void) => safeListen("audio://level", h);
export const setBeatActive = (active: boolean) => safeInvoke("set_beat_active", { active });
export const onVitals = (h: (v: Vitals) => void) => safeListen("vitals://update", h);

// ------------------------------------------------------------------ taskbar speed meter

export interface NetSpeed {
  /** Bytes per second. */
  rx: number;
  tx: number;
  lightTaskbar: boolean;
}

export const onNetSpeed = (h: (s: NetSpeed) => void) => safeListen("netspeed://update", h);

// ------------------------------------------------------------------ Notcher Drop

export interface DropReceived {
  id: number;
  name: string;
  size: number;
  done: number;
  status: "receiving" | "done" | "failed";
}

export interface DropShared {
  id: number;
  name: string;
  size: number;
}

export interface DropState {
  active: boolean;
  url: string | null;
  qr: string | null;
  folder: string;
  received: DropReceived[];
  shared: DropShared[];
  error: string | null;
}

export const dropStart = () => safeInvoke<DropState>("drop_start");
export const dropStop = () => safeInvoke("drop_stop");
export const dropState = () => safeInvoke<DropState>("drop_state");
export const dropShare = (paths: string[]) => safeInvoke<DropState>("drop_share", { paths });
export const dropUnshare = (id: number) => safeInvoke("drop_unshare", { id });
export const dropClearReceived = () => safeInvoke("drop_clear_received");
/** Opens a received file (or the Drop folder when `id` is null); `reveal` shows it in Explorer. */
export const dropOpen = (id: number | null, reveal = false) => safeInvoke("drop_open", { id, reveal });
export const onDropState = (h: (s: DropState) => void) => safeListen("drop://state", h);
export const onDropReceived = (h: (f: { name: string; size: number }) => void) =>
  safeListen("drop://received", h);
