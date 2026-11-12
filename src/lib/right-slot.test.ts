import { describe, expect, it } from "vitest";
import { WEATHER_STALE_MS, rightSlot, weatherUsable } from "./right-slot";
import type { Weather } from "./weather";

const at = new Date(2026, 10, 11, 11, 57, 13);
const w = (over: Partial<Weather> = {}): Weather => ({
  place: "Kolkata", detail: "", approximate: false, temp: 28.4, feels: 30, humidity: 70, wind: 8, code: 1, isDay: true,
  uv: 5, aqi: 42, sunrise: null, sunset: null, hours: [], days: [], at: at.getTime() - 60_000, ...over,
});

describe("right slot", () => {
  it("shows the clock by default", () => {
    expect(rightSlot("clock", at, false, w(), "c")).toEqual({ kind: "clock", primary: "11:57:13", secondary: "AM" });
    expect(rightSlot("clock", at, true, w(), "c")).toEqual({ kind: "clock", primary: "11:57:13", secondary: "" });
  });

  it("shows temperature and AQI when chosen", () => {
    expect(rightSlot("weather_aqi", at, false, w(), "c")).toEqual({ kind: "weather", primary: "28°C", secondary: "AQI 42" });
    expect(rightSlot("weather_aqi", at, false, w(), "f").primary).toBe("83°F");
    expect(rightSlot("weather_aqi", at, false, w({ aqi: null }), "c").secondary).toBe("AQI –");
    expect(rightSlot("weather_aqi", at, false, w({ aqi: undefined }), "c").secondary).toBe("AQI –");
  });

  it("falls back to the clock until weather loads, or when it is stale", () => {
    expect(rightSlot("weather_aqi", at, false, null, "c").kind).toBe("clock");
    expect(rightSlot("weather_aqi", at, false, w({ at: at.getTime() - WEATHER_STALE_MS - 1 }), "c").kind).toBe("clock");
    expect(weatherUsable(w(), at.getTime())).toBe(true);
    expect(weatherUsable(w({ temp: NaN }), at.getTime())).toBe(false);
  });
});
