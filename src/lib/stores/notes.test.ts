import { describe, expect, it, vi } from "vitest";
vi.mock("../bridge", () => ({}));
import { noteTitle, toggleCheck } from "./notes.svelte";

describe("notes helpers", () => {
  it("uses the first non-empty line as the title", () => {
    expect(noteTitle("\n\n# Groceries\nmilk")).toBe("Groceries");
    expect(noteTitle("- [ ] call mum")).toBe("call mum");
    expect(noteTitle("   ")).toBe("Untitled");
  });

  it("toggles checkboxes on one line only", () => {
    const body = "[ ] a\n[x] b\nplain";
    expect(toggleCheck(body, 0)).toBe("[x] a\n[x] b\nplain");
    expect(toggleCheck(body, 1)).toBe("[ ] a\n[ ] b\nplain");
    expect(toggleCheck(body, 2)).toBe(body);
    expect(toggleCheck(body, 9)).toBe(body);
  });
});
