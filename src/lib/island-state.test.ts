import { describe, expect, it } from "vitest";
import { transition, type IslandEvent, type IslandMode } from "./island-state";

const run = (start: IslandMode, events: IslandEvent[]) => events.reduce((m, e) => transition(m, e), start);

describe("island state machine", () => {
  it("opens straight from rest on hover and closes on leave", () => {
    expect(run("rest", [{ type: "hoverIntent" }])).toBe("open");
    expect(run("rest", [{ type: "hoverIntent" }, { type: "leave" }])).toBe("rest");
  });

  it("opens on click", () => {
    expect(run("rest", [{ type: "click" }])).toBe("open");
    expect(run("live", [{ type: "click" }])).toBe("open");
    expect(run("open", [{ type: "click" }])).toBe("open");
  });

  it("hovering a live activity opens the panel", () => {
    expect(run("live", [{ type: "hoverIntent" }])).toBe("open");
  });

  it("toggles with the shortcut from any state", () => {
    expect(run("rest", [{ type: "toggle" }])).toBe("open");
    expect(run("open", [{ type: "toggle" }])).toBe("rest");
    expect(run("live", [{ type: "toggle" }])).toBe("open");
  });

  it("shows live activities only when not in use", () => {
    expect(run("rest", [{ type: "liveStart" }])).toBe("live");
    expect(run("open", [{ type: "liveStart" }])).toBe("open");
    expect(run("live", [{ type: "liveEnd", hovered: false }])).toBe("rest");
    expect(run("live", [{ type: "liveEnd", hovered: true }])).toBe("open");
  });

  it("does not let a stale liveEnd close an open island", () => {
    expect(run("open", [{ type: "liveEnd", hovered: false }])).toBe("open");
  });

  it("leave does not cut a live activity short", () => {
    expect(run("live", [{ type: "leave" }])).toBe("live");
  });

  it("collapses when a full-screen app appears", () => {
    for (const m of ["rest", "live", "open"] as IslandMode[]) {
      expect(run(m, [{ type: "suppress" }])).toBe("rest");
    }
  });

  it("escape closes the panel but not a live activity", () => {
    expect(run("open", [{ type: "escape" }])).toBe("rest");
    expect(run("live", [{ type: "escape" }])).toBe("live");
  });
});
