/**
 * The island's interaction states and the rules for moving between them.
 * Pure functions only; timing lives in the controller.
 */

import type { IslandGeometry } from "./shape";

export type IslandMode = "rest" | "peek" | "live" | "open";

export type IslandEvent =
  /** Pointer has hovered long enough to show intent. */
  | { type: "hoverIntent" }
  /** Pointer has stayed on a peeking island long enough to open it (if enabled). */
  | { type: "dwell" }
  /** Pointer left the island and the grace period passed. */
  | { type: "leave" }
  | { type: "click" }
  /** Global shortcut or tray menu. */
  | { type: "toggle" }
  | { type: "escape" }
  /** A short-lived notification, such as a track change. */
  | { type: "liveStart" }
  | { type: "liveEnd"; hovered: boolean }
  /** A full-screen app appeared. */
  | { type: "suppress" };

export interface TransitionOptions {
  hoverToOpen: boolean;
}

export function transition(
  mode: IslandMode,
  event: IslandEvent,
  options: TransitionOptions,
): IslandMode {
  switch (event.type) {
    case "suppress":
      return "rest";
    case "toggle":
      return mode === "open" ? "rest" : "open";
    case "escape":
      return mode === "open" || mode === "peek" ? "rest" : mode;
    case "click":
      return mode === "open" ? mode : "open";
    case "hoverIntent":
      return mode === "rest" ? "peek" : mode;
    case "dwell":
      return mode === "peek" && options.hoverToOpen ? "open" : mode;
    case "leave":
      return mode === "peek" || mode === "open" ? "rest" : mode;
    case "liveStart":
      return mode === "rest" || mode === "live" ? "live" : mode;
    case "liveEnd":
      if (mode !== "live") return mode;
      return event.hovered ? "peek" : "rest";
  }
}

/** Target geometry for each mode, in CSS pixels. */
export const GEOMETRY: Record<IslandMode, IslandGeometry> = {
  rest: { width: 196, height: 34, radius: 14, shoulder: 10 },
  peek: { width: 300, height: 44, radius: 18, shoulder: 12 },
  live: { width: 380, height: 76, radius: 26, shoulder: 14 },
  open: { width: 600, height: 248, radius: 34, shoulder: 16 },
};

/** Extra hover margin around the resting island so it is easy to reach. */
export const REST_HOT_ZONE = { sides: 24, bottom: 6 };

/** Interaction timings in milliseconds. */
export const TIMING = {
  hoverIntent: 120,
  dwellToOpen: 650,
  leaveGrace: 280,
  liveDuration: 3200,
};
