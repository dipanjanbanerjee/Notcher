/**
 * The island's interaction states and the rules for moving between them.
 * Pure functions only; timing lives in the controller.
 */

import type { IslandGeometry } from "./shape";

/**
 * Two sizes the user controls (rest and open) plus short-lived live activities,
 * such as a track change, that pop out from rest and fold back on their own.
 */
export type IslandMode = "rest" | "live" | "open";

export type IslandEvent =
  /** Pointer has rested on the island long enough to show intent. */
  | { type: "hoverIntent" }
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

export function transition(mode: IslandMode, event: IslandEvent): IslandMode {
  switch (event.type) {
    case "suppress":
      return "rest";
    case "toggle":
      return mode === "open" ? "rest" : "open";
    case "escape":
      return mode === "open" ? "rest" : mode;
    case "click":
    case "hoverIntent":
      return "open";
    case "leave":
      return mode === "open" ? "rest" : mode;
    case "liveStart":
      return mode === "rest" || mode === "live" ? "live" : mode;
    case "liveEnd":
      if (mode !== "live") return mode;
      return event.hovered ? "open" : "rest";
  }
}

/** Target geometry for each mode, in CSS pixels. */
export const GEOMETRY: Record<IslandMode, IslandGeometry> = {
  rest: { width: 252, height: 34, radius: 14, shoulder: 10 },
  live: { width: 380, height: 76, radius: 26, shoulder: 14 },
  open: { width: 640, height: 324, radius: 34, shoulder: 16 },
};

/**
 * Resting shape while a full-screen app is in front: a slim tab at the top edge that
 * stays out of the way but can still be hovered to bring the island back.
 */
export const TUCKED: IslandGeometry = { width: 132, height: 6, radius: 3, shoulder: 4 };

/** Extra hover margin around the resting island so it is easy to reach. */
export const REST_HOT_ZONE = { sides: 24, bottom: 6 };
/** A much smaller margin around the tucked tab, to stay out of the app's way. */
export const TUCKED_HOT_ZONE = { sides: 6, bottom: 3 };

/** Interaction timings in milliseconds. */
export const TIMING = {
  /** Short enough to feel instant, long enough that a cursor passing by doesn't open it. */
  hoverIntent: 130,
  /**
   * Over a maximized or full-screen app the tab sits on top of its title bar or tabs,
   * so the pointer must rest a little longer before the panel opens.
   */
  tuckedIntent: 340,
  leaveGrace: 280,
  liveDuration: 3200,
};
