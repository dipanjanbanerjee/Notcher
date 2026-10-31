import { describe, expect, it, vi } from "vitest";
vi.mock("../bridge", () => ({}));
import { formatBytes, formatRate } from "./vitals.svelte";

describe("byte formatting", () => {
  it("formats sizes", () => {
    expect(formatBytes(0)).toBe("0 B");
    expect(formatBytes(512)).toBe("512 B");
    expect(formatBytes(1536)).toBe("1.5 KB");
    expect(formatBytes(16 * 1024 ** 3)).toBe("16.0 GB");
    expect(formatBytes(250 * 1024 ** 3)).toBe("250 GB");
    expect(formatRate(2048)).toBe("2.0 KB/s");
  });
});
