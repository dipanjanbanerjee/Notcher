/**
 * Weather from Open-Meteo (free, no key). The place comes from the city the user typed,
 * or, when that is empty, from a rough IP lookup.
 */

export type Sky = "clear" | "partly" | "cloudy" | "fog" | "drizzle" | "rain" | "snow" | "storm";
export type TempUnit = "c" | "f";

export interface Place {
  name: string;
  lat: number;
  lon: number;
}

export interface Day {
  /** yyyy-mm-dd in the place's own time zone. */
  date: string;
  code: number;
  max: number;
  min: number;
}

export interface Weather {
  place: string;
  /** Degrees Celsius. */
  temp: number;
  feels: number;
  humidity: number;
  /** km/h */
  wind: number;
  code: number;
  isDay: boolean;
  days: Day[];
  /** When it was fetched, in ms since the epoch. */
  at: number;
}

/** Groups WMO weather codes into the handful of looks we draw. */
export function skyOf(code: number): Sky {
  if (code <= 1) return "clear";
  if (code === 2) return "partly";
  if (code === 3) return "cloudy";
  if (code === 45 || code === 48) return "fog";
  if (code >= 51 && code <= 57) return "drizzle";
  if ((code >= 61 && code <= 67) || (code >= 80 && code <= 82)) return "rain";
  if ((code >= 71 && code <= 77) || code === 85 || code === 86) return "snow";
  if (code >= 95) return "storm";
  return "cloudy";
}

const LABELS: Record<number, string> = {
  0: "Clear",
  1: "Mostly clear",
  2: "Partly cloudy",
  3: "Overcast",
  45: "Foggy",
  48: "Freezing fog",
  51: "Light drizzle",
  53: "Drizzle",
  55: "Heavy drizzle",
  56: "Freezing drizzle",
  57: "Freezing drizzle",
  61: "Light rain",
  63: "Rain",
  65: "Heavy rain",
  66: "Freezing rain",
  67: "Freezing rain",
  71: "Light snow",
  73: "Snow",
  75: "Heavy snow",
  77: "Snow grains",
  80: "Showers",
  81: "Showers",
  82: "Heavy showers",
  85: "Snow showers",
  86: "Snow showers",
  95: "Thunderstorm",
  96: "Storm with hail",
  99: "Storm with hail",
};

export function skyLabel(code: number): string {
  return LABELS[code] ?? "Cloudy";
}

/** The icon for a sky; clear nights get a moon. */
export function skyIcon(code: number, isDay = true) {
  const sky = skyOf(code);
  if (sky === "clear") return isDay ? "sun" : "moon";
  return sky;
}

export function toUnit(celsius: number, unit: TempUnit): number {
  return unit === "f" ? (celsius * 9) / 5 + 32 : celsius;
}

export function formatTemp(celsius: number, unit: TempUnit): string {
  return `${Math.round(toUnit(celsius, unit))}°`;
}

/** "Mon", "Tue"…; today is "Today". `date` is yyyy-mm-dd. */
export function dayName(date: string, today: string): string {
  if (date === today) return "Today";
  const [y, m, d] = date.split("-").map(Number);
  return new Date(y, m - 1, d).toLocaleDateString("en-US", { weekday: "short" });
}

const num = (v: unknown): number | null => {
  const n = typeof v === "string" ? Number(v) : v;
  return typeof n === "number" && Number.isFinite(n) ? n : null;
};

/** Reads an Open-Meteo forecast response; null when it is not one. */
export function parseForecast(json: unknown, place: string, at = Date.now()): Weather | null {
  const j = json as {
    current?: Record<string, unknown>;
    daily?: { time?: unknown[]; weather_code?: unknown[]; temperature_2m_max?: unknown[]; temperature_2m_min?: unknown[] };
  } | null;
  const c = j?.current;
  const temp = num(c?.temperature_2m);
  const code = num(c?.weather_code);
  if (!c || temp === null || code === null) return null;
  const days: Day[] = [];
  const d = j?.daily;
  const times = Array.isArray(d?.time) ? d.time : [];
  times.forEach((t, i) => {
    const dc = num(d?.weather_code?.[i]);
    const max = num(d?.temperature_2m_max?.[i]);
    const min = num(d?.temperature_2m_min?.[i]);
    if (typeof t === "string" && dc !== null && max !== null && min !== null) days.push({ date: t, code: dc, max, min });
  });
  return {
    place,
    temp,
    feels: num(c.apparent_temperature) ?? temp,
    humidity: num(c.relative_humidity_2m) ?? 0,
    wind: num(c.wind_speed_10m) ?? 0,
    code,
    isDay: num(c.is_day) !== 0,
    days,
    at,
  };
}

/** Reads the first hit of an Open-Meteo geocoding search. */
export function parseGeocode(json: unknown): Place | null {
  const hit = (json as { results?: Record<string, unknown>[] } | null)?.results?.[0];
  const lat = num(hit?.latitude);
  const lon = num(hit?.longitude);
  if (!hit || lat === null || lon === null || typeof hit.name !== "string") return null;
  return { name: hit.name, lat, lon };
}

/** Reads an IP lookup (ipwho.is or geojs.io; both use these field names). */
export function parseIpPlace(json: unknown): Place | null {
  const j = json as Record<string, unknown> | null;
  if (!j || j.success === false) return null;
  const lat = num(j.latitude);
  const lon = num(j.longitude);
  if (lat === null || lon === null) return null;
  return { name: typeof j.city === "string" && j.city ? j.city : "Your area", lat, lon };
}

async function getJson(url: string): Promise<unknown> {
  const res = await fetch(url, { signal: AbortSignal.timeout(10_000) });
  if (!res.ok) throw new Error(`HTTP ${res.status}`);
  return res.json();
}

/** Finds the city, or the user's rough location when `city` is empty. */
export async function locate(city: string): Promise<Place> {
  const name = city.trim();
  if (name) {
    const place = parseGeocode(await getJson(`https://geocoding-api.open-meteo.com/v1/search?count=1&language=en&name=${encodeURIComponent(name)}`));
    if (!place) throw new Error(`Couldn't find "${name}"`);
    return place;
  }
  for (const url of ["https://ipwho.is/", "https://get.geojs.io/v1/ip/geo.json"]) {
    try {
      const place = parseIpPlace(await getJson(url));
      if (place) return place;
    } catch {
      /* try the next one */
    }
  }
  throw new Error("Couldn't work out where you are");
}

export async function fetchWeather(place: Place): Promise<Weather> {
  const url =
    `https://api.open-meteo.com/v1/forecast?latitude=${place.lat}&longitude=${place.lon}` +
    "&current=temperature_2m,apparent_temperature,relative_humidity_2m,weather_code,is_day,wind_speed_10m" +
    "&daily=weather_code,temperature_2m_max,temperature_2m_min&timezone=auto&forecast_days=5";
  const weather = parseForecast(await getJson(url), place.name);
  if (!weather) throw new Error("The weather service sent something odd");
  return weather;
}

/** Exchange rates per US dollar, from open.er-api.com (free, no key, daily). */
export interface Rates {
  rates: Record<string, number>;
  /** When the provider last updated them, in ms since the epoch. */
  updated: number;
  at: number;
}

export function parseRates(json: unknown, at = Date.now()): Rates | null {
  const j = json as { result?: string; rates?: Record<string, unknown>; time_last_update_unix?: unknown } | null;
  if (!j || j.result !== "success" || !j.rates) return null;
  const rates: Record<string, number> = {};
  for (const [code, v] of Object.entries(j.rates)) {
    const n = num(v);
    if (n !== null && n > 0 && /^[A-Z]{3}$/.test(code)) rates[code] = n;
  }
  if (!rates.USD) return null;
  return { rates, updated: (num(j.time_last_update_unix) ?? at / 1000) * 1000, at };
}

export async function fetchRates(): Promise<Rates> {
  const rates = parseRates(await getJson("https://open.er-api.com/v6/latest/USD"));
  if (!rates) throw new Error("Couldn't read exchange rates");
  return rates;
}
