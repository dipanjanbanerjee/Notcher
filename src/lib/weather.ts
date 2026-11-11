/**
 * Weather from Open-Meteo (free, no key). The place is one the user picked (a city,
 * area or PIN code), or found automatically: Windows location first, which is
 * accurate, then a rough guess from the internet connection.
 */

export type Sky = "clear" | "partly" | "cloudy" | "fog" | "drizzle" | "rain" | "snow" | "storm";
export type TempUnit = "c" | "f";

export interface Place {
  name: string;
  /** Region, country or postcode, to tell places with the same name apart. */
  detail: string;
  lat: number;
  lon: number;
}

export interface Hour {
  /** Local time, yyyy-mm-ddThh:mm. */
  time: string;
  temp: number;
  code: number;
  isDay: boolean;
  /** Chance of rain, 0–100. */
  rain: number;
}

export interface Day {
  /** yyyy-mm-dd in the place's own time zone. */
  date: string;
  code: number;
  max: number;
  min: number;
  rain: number;
}

export interface Weather {
  place: string;
  detail: string;
  /** True when the place is only a guess from the internet connection. */
  approximate: boolean;
  /**
   * Why the automatic place isn't precise: Windows location is off for desktop apps,
   * Windows couldn't get a fix, or it only knows the rough area. Absent when precise
   * or picked by hand.
   */
  locationNote?: LocationNote | null;
  /** How precise Windows' fix was, in metres, when it gave one. */
  accuracy?: number | null;
  /** Degrees Celsius. */
  temp: number;
  feels: number;
  humidity: number;
  /** km/h */
  wind: number;
  code: number;
  isDay: boolean;
  uv: number | null;
  /** US air quality index (0–500) at the same place; null when it couldn't be fetched. */
  aqi?: number | null;
  /** Local times, hh:mm. */
  sunrise: string | null;
  sunset: string | null;
  hours: Hour[];
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

/** "Now", then "3 PM" or "15:00". `time` is yyyy-mm-ddThh:mm. */
export function hourName(time: string, first: boolean, clock24h: boolean): string {
  if (first) return "Now";
  const h = Number(time.slice(11, 13));
  if (clock24h) return `${String(h).padStart(2, "0")}:00`;
  return `${h % 12 || 12} ${h < 12 ? "AM" : "PM"}`;
}

/** "6:12 AM" or "06:12" from hh:mm. */
export function formatHm(hm: string | null, clock24h: boolean): string {
  if (!hm) return "–";
  if (clock24h) return hm;
  const [h, m] = hm.split(":").map(Number);
  return `${h % 12 || 12}:${String(m).padStart(2, "0")} ${h < 12 ? "AM" : "PM"}`;
}

const num = (v: unknown): number | null => {
  const n = typeof v === "string" && v.trim() !== "" ? Number(v) : v;
  return typeof n === "number" && Number.isFinite(n) ? n : null;
};
const at = (arr: unknown, i: number) => (Array.isArray(arr) ? arr[i] : undefined);

/** Reads an Open-Meteo forecast response; null when it is not one. */
export function parseForecast(json: unknown, place: Pick<Weather, "place" | "detail" | "approximate">, when = Date.now()): Weather | null {
  const j = json as {
    current?: Record<string, unknown>;
    hourly?: Record<string, unknown[]>;
    daily?: Record<string, unknown[]>;
  } | null;
  const c = j?.current;
  const temp = num(c?.temperature_2m);
  const code = num(c?.weather_code);
  if (!c || temp === null || code === null) return null;

  const nowHour = typeof c.time === "string" ? c.time.slice(0, 13) : "";
  const hours: Hour[] = [];
  const h = j?.hourly ?? {};
  (Array.isArray(h.time) ? h.time : []).forEach((t, i) => {
    const ht = num(at(h.temperature_2m, i));
    const hc = num(at(h.weather_code, i));
    if (typeof t !== "string" || ht === null || hc === null || t.slice(0, 13) < nowHour) return;
    hours.push({ time: t, temp: ht, code: hc, isDay: num(at(h.is_day, i)) !== 0, rain: num(at(h.precipitation_probability, i)) ?? 0 });
  });

  const days: Day[] = [];
  const d = j?.daily ?? {};
  (Array.isArray(d.time) ? d.time : []).forEach((t, i) => {
    const dc = num(at(d.weather_code, i));
    const max = num(at(d.temperature_2m_max, i));
    const min = num(at(d.temperature_2m_min, i));
    if (typeof t === "string" && dc !== null && max !== null && min !== null) {
      days.push({ date: t, code: dc, max, min, rain: num(at(d.precipitation_probability_max, i)) ?? 0 });
    }
  });
  const hm = (v: unknown) => (typeof v === "string" && v.length >= 16 ? v.slice(11, 16) : null);

  return {
    ...place,
    temp,
    feels: num(c.apparent_temperature) ?? temp,
    humidity: num(c.relative_humidity_2m) ?? 0,
    wind: num(c.wind_speed_10m) ?? 0,
    code,
    isDay: num(c.is_day) !== 0,
    uv: num(at(d.uv_index_max, 0)),
    sunrise: hm(at(d.sunrise, 0)),
    sunset: hm(at(d.sunset, 0)),
    hours: hours.slice(0, 24),
    days,
    at: when,
  };
}

/** Reads the current US AQI from an Open-Meteo air-quality response; null when missing. */
export function parseAirQuality(json: unknown): number | null {
  const v = num((json as { current?: Record<string, unknown> } | null)?.current?.us_aqi);
  return v === null || v < 0 ? null : Math.round(v);
}

/** Reads Open-Meteo geocoding results. */
export function parseGeocode(json: unknown): Place[] {
  const results = (json as { results?: Record<string, unknown>[] } | null)?.results;
  if (!Array.isArray(results)) return [];
  const out: Place[] = [];
  for (const r of results) {
    const lat = num(r.latitude);
    const lon = num(r.longitude);
    if (lat === null || lon === null || typeof r.name !== "string") continue;
    const detail = [r.admin2 !== r.name ? r.admin2 : null, r.admin1, r.country].filter((x) => typeof x === "string" && x).join(", ");
    out.push({ name: r.name, detail, lat, lon });
  }
  return out;
}

/** Reads OpenStreetMap (Nominatim) search results, used for PIN codes and small areas. */
export function parseNominatim(json: unknown): Place[] {
  if (!Array.isArray(json)) return [];
  const out: Place[] = [];
  for (const r of json as Record<string, unknown>[]) {
    const lat = num(r.lat);
    const lon = num(r.lon);
    if (lat === null || lon === null) continue;
    const a = (r.address ?? {}) as Record<string, string | undefined>;
    const name =
      a.suburb ?? a.neighbourhood ?? a.village ?? a.town ?? a.city_district ?? a.city ?? a.county ?? a.state_district ??
      (typeof r.name === "string" && r.name ? r.name : String(r.display_name ?? "").split(",")[0]);
    if (!name) continue;
    const detail = [a.city && a.city !== name ? a.city : a.state_district, a.state, a.postcode].filter(Boolean).join(", ");
    out.push({ name, detail, lat, lon });
  }
  return out;
}

/** Reads an IP lookup (ipwho.is or geojs.io; both use these field names). */
export function parseIpPlace(json: unknown): Place | null {
  const j = json as Record<string, unknown> | null;
  if (!j || j.success === false) return null;
  const lat = num(j.latitude);
  const lon = num(j.longitude);
  if (lat === null || lon === null) return null;
  const region = typeof j.region === "string" ? j.region : "";
  return { name: typeof j.city === "string" && j.city ? j.city : "Your area", detail: region, lat, lon };
}

/** Reads a BigDataCloud reverse lookup into a place, or null when it names nothing. */
export function parseReverse(json: unknown, lat: number, lon: number): Place | null {
  const j = (json ?? {}) as Record<string, unknown>;
  const str = (k: string) => (typeof j[k] === "string" && j[k] ? (j[k] as string).trim() : "");
  const name = str("locality") || str("city") || str("principalSubdivision");
  if (!name) return null;
  const detail = [str("city") !== name ? str("city") : "", str("principalSubdivision") !== name ? str("principalSubdivision") : "", str("postcode")]
    .filter(Boolean)
    .join(", ");
  return { name, detail, lat, lon };
}

/** Reads a Nominatim reverse lookup into a place at the given coordinates, or null. */
export function parseNominatimReverse(json: unknown, lat: number, lon: number): Place | null {
  if (!json || typeof json !== "object" || "error" in json) return null;
  const [p] = parseNominatim([{ lat, lon, ...(json as Record<string, unknown>) }]);
  return p ? { ...p, lat, lon } : null;
}

/** About 1 km either way: close enough to reuse a place name. */
export const placeKey = (lat: number, lon: number) => `${lat.toFixed(2)},${lon.toFixed(2)}`;

const PLACE_NAMES_KEY = "notcher.placeNames.v1";
type NameCache = Record<string, { name: string; detail: string }>;

function readNames(): NameCache {
  try {
    return (JSON.parse(localStorage.getItem(PLACE_NAMES_KEY) ?? "{}") as NameCache) ?? {};
  } catch {
    return {};
  }
}

function rememberName(key: string, place: Place) {
  try {
    const names = readNames();
    delete names[key];
    names[key] = { name: place.name, detail: place.detail };
    // Keep the most recent few; a laptop only visits so many places.
    const keys = Object.keys(names);
    for (const old of keys.slice(0, Math.max(0, keys.length - 24))) delete names[old];
    localStorage.setItem(PLACE_NAMES_KEY, JSON.stringify(names));
  } catch {
    /* not cached */
  }
}

/**
 * Names the place at these coordinates: from the cache when it was looked up before,
 * else BigDataCloud, then OpenStreetMap, then the city an IP lookup gives when it is
 * nearby. Null when nothing could name it.
 */
export async function nameCoordinates(lat: number, lon: number, ipGuess?: () => Promise<Place | null>): Promise<Place | null> {
  const key = placeKey(lat, lon);
  const cached = readNames()[key];
  if (cached?.name) return { ...cached, lat, lon };
  const lookups: (() => Promise<Place | null>)[] = [
    () =>
      getJson(`https://api.bigdatacloud.net/data/reverse-geocode-client?localityLanguage=en&latitude=${lat}&longitude=${lon}`).then((j) =>
        parseReverse(j, lat, lon),
      ),
    () =>
      getJson(
        `https://nominatim.openstreetmap.org/reverse?format=jsonv2&zoom=14&addressdetails=1&accept-language=en&lat=${lat}&lon=${lon}`,
      ).then((j) => parseNominatimReverse(j, lat, lon)),
  ];
  for (const lookup of lookups) {
    const place = await lookup().catch(() => null);
    if (place) {
      rememberName(key, place);
      return place;
    }
  }
  // Last resort: the IP lookup's city, if it is around here (within about 100 km).
  const ip = await ipGuess?.().catch(() => null);
  if (ip && Math.abs(ip.lat - lat) < 1 && Math.abs(ip.lon - lon) < 1) return { ...ip, lat, lon };
  return null;
}

async function ipPlace(): Promise<Place | null> {
  for (const url of ["https://ipwho.is/", "https://get.geojs.io/v1/ip/geo.json"]) {
    try {
      const place = parseIpPlace(await getJson(url));
      if (place) return place;
    } catch {
      /* try the next one */
    }
  }
  return null;
}

/** Postcodes: digits, or letters and digits like "SW1A 1AA". */
export function looksLikePostcode(q: string): boolean {
  const t = q.trim();
  return /^\d{3,10}$/.test(t) || (/\d/.test(t) && /^[A-Za-z0-9 -]{3,10}$/.test(t) && !/^[A-Za-z]+$/.test(t));
}

async function getJson(url: string): Promise<unknown> {
  const res = await fetch(url, { signal: AbortSignal.timeout(10_000) });
  if (!res.ok) throw new Error(`HTTP ${res.status}`);
  return res.json();
}

const nominatim = (params: string) =>
  getJson(`https://nominatim.openstreetmap.org/search?format=jsonv2&addressdetails=1&limit=6&accept-language=en&${params}`)
    .then(parseNominatim)
    .catch(() => [] as Place[]);

const near = (a: Place, b: Place) => Math.abs(a.lat - b.lat) < 0.02 && Math.abs(a.lon - b.lon) < 0.02;

/** Places matching a city, area or postcode, best first. */
export async function searchPlaces(query: string): Promise<Place[]> {
  const q = query.trim();
  if (q.length < 2) return [];
  const meteo = getJson(`https://geocoding-api.open-meteo.com/v1/search?count=6&language=en&name=${encodeURIComponent(q)}`)
    .then(parseGeocode)
    .catch(() => [] as Place[]);
  let found: Place[];
  if (looksLikePostcode(q)) {
    const compact = q.replace(/\s+/g, "");
    // Six digits is almost always an Indian PIN code; ask for India first.
    const [osm, osmIn, om] = await Promise.all([
      nominatim(`postalcode=${encodeURIComponent(q)}`),
      /^\d{6}$/.test(compact) ? nominatim(`postalcode=${compact}&countrycodes=in`) : Promise.resolve([]),
      meteo,
    ]);
    found = [...osmIn, ...osm, ...om];
  } else {
    found = await meteo;
    if (!found.length) found = await nominatim(`q=${encodeURIComponent(q)}`);
  }
  const unique: Place[] = [];
  for (const p of found) if (!unique.some((u) => near(u, p))) unique.push(p);
  return unique.slice(0, 6);
}

export type LocationNote = "off" | "unavailable" | "coarse";

/** What Windows said when asked where the PC is. */
export type WindowsLocation =
  | { position: { lat: number; lon: number; accuracy: number } }
  | { error: { kind: "off" | "unavailable"; message: string } };

/** A Windows fix less precise than this (metres) is only the rough area. */
export const COARSE_FIX_M = 3000;

/** How good the Windows answer is: null when it is a precise fix. */
export function locationNoteOf(found: WindowsLocation): LocationNote | null {
  if ("error" in found) return found.error.kind;
  return found.position.accuracy > COARSE_FIX_M ? "coarse" : null;
}

export interface AutoPlace {
  place: Place;
  approximate: boolean;
  note: LocationNote | null;
  accuracy: number | null;
}

/**
 * Where the PC is: Windows' own fix when there is one, named by a reverse lookup;
 * otherwise a rough guess from the internet connection.
 */
export async function locateAutomatically(found: WindowsLocation): Promise<AutoPlace> {
  const note = locationNoteOf(found);
  if ("position" in found) {
    const { lat, lon, accuracy } = found.position;
    const place = (await nameCoordinates(lat, lon, ipPlace)) ?? { name: "Your location", detail: "", lat, lon };
    return { place, approximate: note !== null, note, accuracy };
  }
  const place = await ipPlace();
  if (place) return { place, approximate: true, note, accuracy: null };
  throw new Error(
    note === "off"
      ? "Couldn't work out where you are. Turn on Windows location for desktop apps, or pick a place"
      : "Couldn't work out where you are. Pick a place instead",
  );
}

export async function fetchWeather(place: Place, approximate: boolean, extra: Pick<Weather, "locationNote" | "accuracy"> = {}): Promise<Weather> {
  const url =
    `https://api.open-meteo.com/v1/forecast?latitude=${place.lat}&longitude=${place.lon}` +
    "&current=temperature_2m,apparent_temperature,relative_humidity_2m,weather_code,is_day,wind_speed_10m" +
    "&hourly=temperature_2m,weather_code,precipitation_probability,is_day" +
    "&daily=weather_code,temperature_2m_max,temperature_2m_min,precipitation_probability_max,sunrise,sunset,uv_index_max" +
    "&timezone=auto&forecast_days=7";
  // Air quality comes from Open-Meteo's sister service for the same coordinates. It is
  // fetched alongside and never holds up or fails the weather itself.
  const airUrl = `https://air-quality-api.open-meteo.com/v1/air-quality?latitude=${place.lat}&longitude=${place.lon}&current=us_aqi`;
  const [forecast, aqi] = await Promise.all([getJson(url), getJson(airUrl).then(parseAirQuality, () => null)]);
  const weather = parseForecast(forecast, { place: place.name, detail: place.detail, approximate });
  if (!weather) throw new Error("The weather service sent something odd");
  return { ...weather, aqi, ...extra };
}

/** Exchange rates per US dollar, from open.er-api.com (free, no key, daily). */
export interface Rates {
  rates: Record<string, number>;
  /** When the provider last updated them, in ms since the epoch. */
  updated: number;
  at: number;
}

export function parseRates(json: unknown, when = Date.now()): Rates | null {
  const j = json as { result?: string; rates?: Record<string, unknown>; time_last_update_unix?: unknown } | null;
  if (!j || j.result !== "success" || !j.rates) return null;
  const rates: Record<string, number> = {};
  for (const [code, v] of Object.entries(j.rates)) {
    const n = num(v);
    if (n !== null && n > 0 && /^[A-Z]{3}$/.test(code)) rates[code] = n;
  }
  if (!rates.USD) return null;
  return { rates, updated: (num(j.time_last_update_unix) ?? when / 1000) * 1000, at: when };
}

export async function fetchRates(): Promise<Rates> {
  const rates = parseRates(await getJson("https://open.er-api.com/v6/latest/USD"));
  if (!rates) throw new Error("Couldn't read exchange rates");
  return rates;
}
