import { describe, expect, it } from "vitest";
import { dayName, formatTemp, parseForecast, parseGeocode, parseIpPlace, parseRates, skyIcon, skyOf } from "./weather";

describe("weather", () => {
  it("groups weather codes", () => {
    expect(skyOf(0)).toBe("clear");
    expect(skyOf(2)).toBe("partly");
    expect(skyOf(45)).toBe("fog");
    expect(skyOf(53)).toBe("drizzle");
    expect(skyOf(81)).toBe("rain");
    expect(skyOf(86)).toBe("snow");
    expect(skyOf(99)).toBe("storm");
    expect(skyIcon(0, false)).toBe("moon");
    expect(skyIcon(1, true)).toBe("sun");
  });

  it("formats temperatures in either unit", () => {
    expect(formatTemp(21.6, "c")).toBe("22°");
    expect(formatTemp(100, "f")).toBe("212°");
  });

  it("names days", () => {
    expect(dayName("2026-10-31", "2026-10-31")).toBe("Today");
    expect(dayName("2026-11-01", "2026-10-31")).toBe("Sun");
  });

  it("reads a forecast", () => {
    const w = parseForecast(
      {
        current: { temperature_2m: 29.4, apparent_temperature: 33, relative_humidity_2m: 70, weather_code: 2, is_day: 0, wind_speed_10m: 9 },
        daily: {
          time: ["2026-10-31", "2026-11-01"],
          weather_code: [2, 61],
          temperature_2m_max: [31, 30],
          temperature_2m_min: [24, null],
        },
      },
      "Kolkata",
      5,
    );
    expect(w).toMatchObject({ place: "Kolkata", temp: 29.4, feels: 33, humidity: 70, code: 2, isDay: false, at: 5 });
    expect(w?.days).toEqual([{ date: "2026-10-31", code: 2, max: 31, min: 24 }]);
    expect(parseForecast({ error: true }, "x")).toBeNull();
  });

  it("reads places", () => {
    expect(parseGeocode({ results: [{ name: "Pune", latitude: 18.5, longitude: 73.8 }] })).toEqual({ name: "Pune", lat: 18.5, lon: 73.8 });
    expect(parseGeocode({})).toBeNull();
    expect(parseIpPlace({ success: true, city: "Delhi", latitude: 28.6, longitude: 77.2 })).toEqual({ name: "Delhi", lat: 28.6, lon: 77.2 });
    expect(parseIpPlace({ city: "Delhi", latitude: "28.6", longitude: "77.2" })).toEqual({ name: "Delhi", lat: 28.6, lon: 77.2 });
    expect(parseIpPlace({ success: false })).toBeNull();
  });

  it("reads exchange rates", () => {
    const r = parseRates({ result: "success", time_last_update_unix: 100, rates: { USD: 1, INR: 83.2, bad: 2, XXX: -1 } }, 7);
    expect(r).toEqual({ rates: { USD: 1, INR: 83.2 }, updated: 100_000, at: 7 });
    expect(parseRates({ result: "error" })).toBeNull();
  });
});
