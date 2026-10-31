import { describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/plugin-store", () => ({ load: vi.fn() }));

import { DEFAULT_SETTINGS, sanitize } from "./settings";

describe("settings", () => {
  it("fills missing keys with defaults", () => {
    expect(sanitize({})).toEqual(DEFAULT_SETTINGS);
  });

  it("drops wrong types and unknown themes", () => {
    const s = sanitize({ glow: "yes", theme: "rainbow", clock24h: true, extra: 1 });
    expect(s.glow).toBe(DEFAULT_SETTINGS.glow);
    expect(s.theme).toBe("obsidian");
    expect(s.clock24h).toBe(true);
    expect("extra" in s).toBe(false);
  });
});

describe("water reminder setting", () => {
  it("defaults to an hour and only accepts the offered choices", () => {
    expect(sanitize({}).waterReminder).toBe(60);
    expect(sanitize({ waterReminder: 0 }).waterReminder).toBe(0);
    expect(sanitize({ waterReminder: 45 }).waterReminder).toBe(45);
    expect(sanitize({ waterReminder: 7 }).waterReminder).toBe(60);
    expect(sanitize({ waterReminder: "30" }).waterReminder).toBe(60);
  });
});
