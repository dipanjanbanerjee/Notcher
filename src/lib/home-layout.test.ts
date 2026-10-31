import { describe, expect, it } from "vitest";
import {
  DEFAULT_LAYOUT,
  addWidget,
  canAdd,
  cellsUsed,
  moveWidget,
  placeWidget,
  removeWidget,
  sanitizeLayout,
  upgradeLayout,
} from "./home-layout";

describe("home layout", () => {
  it("default fills the grid", () => {
    expect(cellsUsed(DEFAULT_LAYOUT)).toBe(8);
  });

  it("only adds widgets that fit and aren't shown yet", () => {
    expect(canAdd(DEFAULT_LAYOUT, "note")).toBe(false);
    const fewer = removeWidget(DEFAULT_LAYOUT, "note");
    expect(canAdd(fewer, "note")).toBe(true);
    expect(addWidget(fewer, "note")).toEqual(DEFAULT_LAYOUT);
    expect(addWidget(fewer, "clock")).toBe(fewer);
    expect(canAdd(removeWidget(DEFAULT_LAYOUT, "focus"), "media")).toBe(false);
  });

  it("gives older saved layouts the screen time widget once, if it fits", () => {
    const old = ["clock", "media", "focus", "system", "clipboard"] as const;
    expect(upgradeLayout([...old])).toEqual(["clock", "media", "screen", "focus", "system", "clipboard"]);
    expect(upgradeLayout(["clock", "screen"])).toEqual(["clock", "screen"]);
    const full = ["clock", "media", "focus", "system", "clipboard", "note", "media"] as const;
    expect(upgradeLayout(sanitizeLayout([...full, "screen"]))).toContain("screen");
  });

  it("moves widgets within bounds", () => {
    expect(moveWidget(DEFAULT_LAYOUT, "media", -1).slice(0, 2)).toEqual(["media", "clock"]);
    expect(moveWidget(DEFAULT_LAYOUT, "clock", -1)).toBe(DEFAULT_LAYOUT);
    expect(moveWidget(DEFAULT_LAYOUT, "note", 1)).toBe(DEFAULT_LAYOUT);
  });

  it("cleans stored layouts", () => {
    expect(sanitizeLayout(["media", "media", "bogus", "note"])).toEqual(["media", "note"]);
    expect(sanitizeLayout("nope")).toEqual(DEFAULT_LAYOUT);
    expect(cellsUsed(sanitizeLayout(["clock", "media", "focus", "system", "clipboard", "note"]))).toBe(7);
  });
  it("drag places a widget at a new index", () => {
    const l = ["clock", "media", "focus", "system"] as const;
    expect(placeWidget([...l], "focus", 0)).toEqual(["focus", "clock", "media", "system"]);
    expect(placeWidget([...l], "clock", 3)).toEqual(["media", "focus", "system", "clock"]);
    expect(placeWidget([...l], "clock", 99)).toEqual(["media", "focus", "system", "clock"]);
    expect(placeWidget([...l], "clock", 0)).toEqual([...l]);
  });
});
