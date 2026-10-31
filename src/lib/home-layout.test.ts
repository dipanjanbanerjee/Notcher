import { describe, expect, it } from "vitest";
import {
  DEFAULT_LAYOUT,
  addWidget,
  canAdd,
  cellsUsed,
  moveWidget,
  removeWidget,
  sanitizeLayout,
} from "./home-layout";

describe("home layout", () => {
  it("default fills the grid", () => {
    expect(cellsUsed(DEFAULT_LAYOUT)).toBe(6);
  });

  it("only adds widgets that fit and aren't shown yet", () => {
    expect(canAdd(DEFAULT_LAYOUT, "note")).toBe(false);
    const fewer = removeWidget(DEFAULT_LAYOUT, "system");
    expect(canAdd(fewer, "note")).toBe(true);
    expect(addWidget(fewer, "note")).toEqual(["clock", "media", "focus", "clipboard", "note"]);
    expect(addWidget(fewer, "clock")).toBe(fewer);
  });

  it("moves widgets within bounds", () => {
    expect(moveWidget(DEFAULT_LAYOUT, "media", -1).slice(0, 2)).toEqual(["media", "clock"]);
    expect(moveWidget(DEFAULT_LAYOUT, "clock", -1)).toBe(DEFAULT_LAYOUT);
    expect(moveWidget(DEFAULT_LAYOUT, "clipboard", 1)).toBe(DEFAULT_LAYOUT);
  });

  it("cleans stored layouts", () => {
    expect(sanitizeLayout(["media", "media", "bogus", "note"])).toEqual(["media", "note"]);
    expect(sanitizeLayout("nope")).toEqual(DEFAULT_LAYOUT);
    expect(cellsUsed(sanitizeLayout(["clock", "media", "focus", "system", "clipboard", "note"]))).toBe(6);
  });
});
