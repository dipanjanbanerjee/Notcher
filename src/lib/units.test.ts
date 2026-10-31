import { describe, expect, it } from "vitest";
import { convert, formatNumber } from "./units";

describe("unit conversion", () => {
  it("converts by factor", () => {
    expect(convert("length", 1, "mi", "km")).toBeCloseTo(1.609344, 6);
    expect(convert("weight", 1, "kg", "lb")).toBeCloseTo(2.20462, 4);
    expect(convert("data", 1, "gb", "mb")).toBe(1024);
    expect(convert("time", 2, "h", "min")).toBe(120);
    expect(convert("speed", 36, "kmh", "mps")).toBeCloseTo(10, 9);
  });

  it("handles temperature offsets", () => {
    expect(convert("temperature", 100, "c", "f")).toBeCloseTo(212, 9);
    expect(convert("temperature", 32, "f", "c")).toBeCloseTo(0, 9);
    expect(convert("temperature", 0, "k", "c")).toBeCloseTo(-273.15, 9);
  });

  it("uses rates for currency and refuses unknowns", () => {
    const rates = { USD: 1, INR: 83, EUR: 0.92 };
    expect(convert("currency", 2, "USD", "INR", rates)).toBeCloseTo(166, 9);
    expect(convert("currency", 83, "INR", "EUR", rates)).toBeCloseTo(0.92, 9);
    expect(convert("currency", 1, "USD", "XYZ", rates)).toBeNull();
    expect(convert("length", 1, "m", "parsec")).toBeNull();
    expect(convert("length", Number.NaN, "m", "km")).toBeNull();
  });

  it("formats results tidily", () => {
    expect(formatNumber(1.609344)).toBe("1.60934");
    expect(formatNumber(1234567)).toBe("1,234,567");
    expect(formatNumber(0.5)).toBe("0.5");
    expect(formatNumber(null)).toBe("—");
    expect(formatNumber(0)).toBe("0");
  });
});
