import { describe, expect, it } from "vitest";
import {
  dayName,
  formatHm,
  formatTemp,
  hourName,
  COARSE_FIX_M,
  locationNoteOf,
  looksLikePostcode,
  nameCoordinates,
  parseForecast,
  parseGeocode,
  parseIpPlace,
  parseNominatim,
  parseNominatimReverse,
  parseAirQuality,
  parseRates,
  placeKey,
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
    expect(parseReverse(null, 1, 2)).toBeNull();
    expect(parseReverse({ locality: "", city: "" }, 1, 2)).toBeNull();
    expect(
      parseNominatimReverse({ address: { suburb: "Salt Lake", city: "Bidhannagar", state: "West Bengal", postcode: "700091" } }, 22.5812, 88.4123),
    ).toEqual({ name: "Salt Lake", detail: "Bidhannagar, West Bengal, 700091", lat: 22.5812, lon: 88.4123 });
    expect(parseNominatimReverse({ error: "Unable to geocode" }, 1, 2)).toBeNull();
  });

  it("names coordinates from the cache, a lookup, or a nearby IP guess", async () => {
    const store = new Map<string, string>();
    const realFetch = globalThis.fetch;
    const realStorage = globalThis.localStorage;
    Object.defineProperty(globalThis, "localStorage", {
      configurable: true,
      value: { getItem: (k: string) => store.get(k) ?? null, setItem: (k: string, v: string) => store.set(k, v) },
    });
    let calls = 0;
    const respond = (body: unknown) => Promise.resolve({ ok: true, json: () => Promise.resolve(body) } as Response);
    globalThis.fetch = ((url: string) => {
      calls++;
      if (url.includes("bigdatacloud")) return respond({ locality: "" });
      return respond({ address: { suburb: "Salt Lake", city: "Bidhannagar", state: "West Bengal" } });
    }) as typeof fetch;
    try {
      const first = await nameCoordinates(22.581, 88.412);
      expect(first?.name).toBe("Salt Lake");
      expect(calls).toBe(2);
      // Looked up once; the next refresh nearby reads the cache.
      expect((await nameCoordinates(22.583, 88.414))?.name).toBe("Salt Lake");
      expect(calls).toBe(2);
      expect(placeKey(22.581, 88.412)).toBe("22.58,88.41");

      globalThis.fetch = (() => Promise.reject(new Error("offline"))) as typeof fetch;
      const ip = async () => ({ name: "Kolkata", detail: "West Bengal", lat: 22.57, lon: 88.36 });
      expect((await nameCoordinates(10, 10, ip))).toBeNull();
      expect((await nameCoordinates(22.9, 88.1, ip))?.name).toBe("Kolkata");
    } finally {
      globalThis.fetch = realFetch;
      Object.defineProperty(globalThis, "localStorage", { configurable: true, value: realStorage });
    }
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

describe("locationNoteOf", () => {
  const fix = (accuracy: number) => ({ position: { lat: 22.57, lon: 88.36, accuracy } });
  it("treats a close Windows fix as precise", () => {
    expect(locationNoteOf(fix(35))).toBeNull();
    expect(locationNoteOf(fix(COARSE_FIX_M))).toBeNull();
  });
  it("flags a city-sized Windows fix as the rough area", () => {
    expect(locationNoteOf(fix(12_000))).toBe("coarse");
  });
  it("tells location that is switched off from one that found nothing", () => {
    expect(locationNoteOf({ error: { kind: "off", message: "" } })).toBe("off");
    expect(locationNoteOf({ error: { kind: "unavailable", message: "" } })).toBe("unavailable");
  });
});

describe("air quality", () => {
  it("reads the current US AQI", () => {
    expect(parseAirQuality({ current: { time: "2026-11-11T15:00", us_aqi: 42.4 } })).toBe(42);
    expect(parseAirQuality({ current: { us_aqi: "153" } })).toBe(153);
  });

  it("is null when missing or nonsense", () => {
    expect(parseAirQuality(null)).toBeNull();
    expect(parseAirQuality({ current: {} })).toBeNull();
    expect(parseAirQuality({ current: { us_aqi: null } })).toBeNull();
    expect(parseAirQuality({ current: { us_aqi: -1 } })).toBeNull();
    expect(parseAirQuality({ error: true, reason: "bad" })).toBeNull();
  });
});
