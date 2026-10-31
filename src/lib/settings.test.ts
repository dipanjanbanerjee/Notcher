import { describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/plugin-store", () => ({ load: vi.fn() }));

import { DEFAULT_SETTINGS, sanitize } from "./settings";

describe("settings", () => {
  it("fills missing keys with defaults", () => {
    expect(sanitize({})).toEqual(DEFAULT_SETTINGS);
  });

  it("drops wrong types and unknown themes", () => {
    const s = sanitize({ glow: "yes", theme: "rainbow", hoverToOpen: true, extra: 1 });
    expect(s.glow).toBe(DEFAULT_SETTINGS.glow);
    expect(s.theme).toBe("obsidian");
    expect(s.hoverToOpen).toBe(true);
    expect("extra" in s).toBe(false);
  });
});
