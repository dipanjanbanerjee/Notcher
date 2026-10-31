import { describe, expect, it } from "vitest";
import {
  dayName,
  formatHm,
  formatTemp,
  hourName,
  looksLikePostcode,
  parseForecast,
  parseGeocode,
  parseIpPlace,
  parseNominatim,
  parseRates,
  parseReverse,
  skyIcon,
  skyOf,
} from "./weather";

const place = { place: "Kolkata", detail: "West Bengal", approximate: false };

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

  it("formats temperatures, days and hours", () => {
    expect(formatTemp(21.6, "c")).toBe("22°");
    expect(formatTemp(100, "f")).toBe("212°");
    expect(dayName("2026-10-31", "2026-10-31")).toBe("Today");
    expect(dayName("2026-11-01", "2026-10-31")).toBe("Sun");
    expect(hourName("2026-10-31T15:00", true, false)).toBe("Now");
    expect(hourName("2026-10-31T15:00", false, false)).toBe("3 PM");
    expect(hourName("2026-10-31T00:00", false, false)).toBe("12 AM");
    expect(hourName("2026-10-31T07:00", false, true)).toBe("07:00");
    expect(formatHm("05:42", false)).toBe("5:42 AM");
    expect(formatHm("17:05", true)).toBe("17:05");
    expect(formatHm(null, true)).toBe("–");
  });

  it("reads a forecast from the current hour on", () => {
    const w = parseForecast(
      {
        current: { time: "2026-10-31T15:15", temperature_2m: 29.4, apparent_temperature: 33, relative_humidity_2m: 70, weather_code: 2, is_day: 1, wind_speed_10m: 9 },
        hourly: {
          time: ["2026-10-31T14:00", "2026-10-31T15:00", "2026-10-31T16:00"],
          temperature_2m: [30, 29, 28],
          weather_code: [1, 2, 61],
          precipitation_probability: [0, 10, 60],
          is_day: [1, 1, 1],
        },
        daily: {
          time: ["2026-10-31", "2026-11-01"],
          weather_code: [2, 61],
          temperature_2m_max: [31, 30],
          temperature_2m_min: [24, null],
          precipitation_probability_max: [40, 80],
          sunrise: ["2026-10-31T05:42"],
          sunset: ["2026-10-31T17:03"],
          uv_index_max: [6.5],
        },
      },
      place,
      5,
    );
    expect(w).toMatchObject({ place: "Kolkata", temp: 29.4, feels: 33, humidity: 70, code: 2, isDay: true, uv: 6.5, sunrise: "05:42", sunset: "17:03", at: 5 });
    expect(w?.hours.map((h) => h.time)).toEqual(["2026-10-31T15:00", "2026-10-31T16:00"]);
    expect(w?.hours[1].rain).toBe(60);
    expect(w?.days).toEqual([{ date: "2026-10-31", code: 2, max: 31, min: 24, rain: 40 }]);
    expect(parseForecast({ error: true }, place)).toBeNull();
  });

  it("reads places from every source", () => {
    expect(parseGeocode({ results: [{ name: "Pune", admin1: "Maharashtra", country: "India", latitude: 18.5, longitude: 73.8 }] })).toEqual([
      { name: "Pune", detail: "Maharashtra, India", lat: 18.5, lon: 73.8 },
    ]);
    expect(parseGeocode({})).toEqual([]);
    expect(
      parseNominatim([{ lat: "22.58", lon: "88.41", address: { suburb: "Salt Lake", city: "Bidhannagar", state: "West Bengal", postcode: "700091" } }]),
    ).toEqual([{ name: "Salt Lake", detail: "Bidhannagar, West Bengal, 700091", lat: 22.58, lon: 88.41 }]);
    expect(parseIpPlace({ success: true, city: "Delhi", region: "Delhi", latitude: 28.6, longitude: 77.2 })).toEqual({ name: "Delhi", detail: "Delhi", lat: 28.6, lon: 77.2 });
    expect(parseIpPlace({ success: false })).toBeNull();
    expect(parseReverse({ locality: "Salt Lake", city: "Kolkata", principalSubdivision: "West Bengal", postcode: "700091" }, 1, 2)).toEqual({
      name: "Salt Lake",
      detail: "Kolkata, West Bengal, 700091",
      lat: 1,
      lon: 2,
    });
    expect(parseReverse(null, 1, 2).name).toBe("Your location");
  });

  it("spots postcodes", () => {
    expect(looksLikePostcode("700091")).toBe(true);
    expect(looksLikePostcode("SW1A 1AA")).toBe(true);
    expect(looksLikePostcode("10001")).toBe(true);
    expect(looksLikePostcode("Kolkata")).toBe(false);
    expect(looksLikePostcode("New York")).toBe(false);
  });

  it("reads exchange rates", () => {
    const r = parseRates({ result: "success", time_last_update_unix: 100, rates: { USD: 1, INR: 83.2, bad: 2, XXX: -1 } }, 7);
    expect(r).toEqual({ rates: { USD: 1, INR: 83.2 }, updated: 100_000, at: 7 });
    expect(parseRates({ result: "error" })).toBeNull();
  });
});
