import { describe, expect, it } from "vitest";
import { GEOMETRY, TUCKED, inputRegion, transition, type IslandEvent, type IslandMode } from "./island-state";

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

describe("input region", () => {
  const C = 380;
  const inside = (r: { x: number; y: number; width: number; height: number }, x: number, y: number) =>
    x >= r.x && x < r.x + r.width && y >= r.y && y < r.y + r.height;
  const contains = (outer: ReturnType<typeof inputRegion>, inner: ReturnType<typeof inputRegion>) =>
    inner.x >= outer.x &&
    inner.y >= outer.y &&
    inner.x + inner.width <= outer.x + outer.width &&
    inner.y + inner.height <= outer.y + outer.height;

  it("nests, so leaving a bigger state never lands inside a smaller one", () => {
    const tucked = inputRegion(TUCKED, "rest", true, C);
    const rest = inputRegion(GEOMETRY.rest, "rest", false, C);
    const live = inputRegion(GEOMETRY.live, "live", false, C);
    const open = inputRegion(GEOMETRY.open, "open", false, C);
    expect(contains(rest, tucked)).toBe(true);
    expect(contains(live, rest)).toBe(true);
    expect(contains(open, live)).toBe(true);
  });

  it("does not reopen the panel for a pointer resting just outside it", () => {
    const open = inputRegion(GEOMETRY.open, "open", false, C);
    // Just below and just beside the open panel's region.
    const below = { x: C, y: open.y + open.height + 2 };
    const beside = { x: open.x + open.width + 2, y: 100 };
    for (const suppressed of [false, true]) {
      // The panel is closing and its shape is still almost fully open.
      const closing = inputRegion(GEOMETRY.open, "rest", suppressed, C);
      expect(inside(closing, below.x, below.y)).toBe(false);
      expect(inside(closing, beside.x, beside.y)).toBe(false);
    }
  });

  it("covers the panel's small overshoot while opening", () => {
    const open = inputRegion(GEOMETRY.rest, "open", false, C);
    const overshoot = { ...GEOMETRY.open, width: GEOMETRY.open.width + 12, height: GEOMETRY.open.height + 8 };
    expect(inside(open, C, overshoot.height - 1)).toBe(true);
    expect(inside(open, C + overshoot.width / 2 - 1, 100)).toBe(true);
  });

  it("covers the current shape and the destination while growing", () => {
    const r = inputRegion(GEOMETRY.rest, "live", false, C);
    expect(r.width).toBeGreaterThanOrEqual(GEOMETRY.live.width);
    expect(r.height).toBeGreaterThanOrEqual(GEOMETRY.live.height);
  });
});
