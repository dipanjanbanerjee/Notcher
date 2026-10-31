import { describe, expect, it } from "vitest";
import { estimatePosition, formatClock, formatDuration, formatShortDate } from "./time";

describe("time helpers", () => {
  it("formats durations", () => {
    expect(formatDuration(0)).toBe("0:00");
    expect(formatDuration(187_000)).toBe("3:07");
    expect(formatDuration(3_765_000)).toBe("1:02:45");
    expect(formatDuration(-5)).toBe("0:00");
    expect(formatDuration(NaN)).toBe("0:00");
  });

  it("formats the clock", () => {
    const d = new Date(2026, 0, 1, 0, 5);
    expect(formatClock(d, false)).toBe("12:05");
    expect(formatClock(d, true)).toBe("00:05");
    expect(formatClock(new Date(2026, 0, 1, 15, 30), false)).toBe("3:30");
  });

  it("formats seconds and the short date", () => {
    const d = new Date(2026, 9, 31, 22, 52, 7);
    expect(formatClock(d, false, true)).toBe("10:52:07");
    expect(formatClock(d, true, true)).toBe("22:52:07");
    expect(formatShortDate(d, "en-US")).toBe("Sat Oct 31");
  });

  it("estimates playback position", () => {
    expect(estimatePosition(1000, 0, 500, true, 10_000)).toBe(1500);
    expect(estimatePosition(1000, 0, 500, false, 10_000)).toBe(1000);
    expect(estimatePosition(9_900, 0, 5000, true, 10_000)).toBe(10_000);
    expect(estimatePosition(1000, 1000, 500, true, 0)).toBe(1000);
  });
});
