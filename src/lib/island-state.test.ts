import { describe, expect, it } from "vitest";
import { transition, type IslandEvent, type IslandMode } from "./island-state";

const run = (start: IslandMode, events: IslandEvent[], hoverToOpen = false) =>
  events.reduce((m, e) => transition(m, e, { hoverToOpen }), start);

describe("island state machine", () => {
  it("peeks on hover and closes on leave", () => {
    expect(run("rest", [{ type: "hoverIntent" }])).toBe("peek");
    expect(run("rest", [{ type: "hoverIntent" }, { type: "leave" }])).toBe("rest");
  });

  it("opens on click and on dwell only when hover-to-open is on", () => {
    expect(run("peek", [{ type: "click" }])).toBe("open");
    expect(run("peek", [{ type: "dwell" }], false)).toBe("peek");
    expect(run("peek", [{ type: "dwell" }], true)).toBe("open");
  });

  it("toggles with the shortcut from any state", () => {
    expect(run("rest", [{ type: "toggle" }])).toBe("open");
    expect(run("open", [{ type: "toggle" }])).toBe("rest");
    expect(run("live", [{ type: "toggle" }])).toBe("open");
  });

  it("shows live activities only when not in use", () => {
    expect(run("rest", [{ type: "liveStart" }])).toBe("live");
    expect(run("open", [{ type: "liveStart" }])).toBe("open");
    expect(run("peek", [{ type: "liveStart" }])).toBe("peek");
    expect(run("live", [{ type: "liveEnd", hovered: false }])).toBe("rest");
    expect(run("live", [{ type: "liveEnd", hovered: true }])).toBe("peek");
  });

  it("does not let a stale liveEnd close an open island", () => {
    expect(run("open", [{ type: "liveEnd", hovered: false }])).toBe("open");
  });

  it("collapses when a full-screen app appears", () => {
    for (const m of ["rest", "peek", "live", "open"] as IslandMode[]) {
      expect(run(m, [{ type: "suppress" }])).toBe("rest");
    }
  });

  it("escape closes open and peek but not live", () => {
    expect(run("open", [{ type: "escape" }])).toBe("rest");
    expect(run("peek", [{ type: "escape" }])).toBe("rest");
    expect(run("live", [{ type: "escape" }])).toBe("live");
  });
});
