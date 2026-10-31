import { inTauri, notifySettingsChanged, preciseLocation } from "../bridge";
import { loadSettings, saveSetting } from "../settings";
import { fetchRates, fetchWeather, locateAutomatically, searchPlaces, type Place, type Rates, type TempUnit, type Weather } from "../weather";

const WEATHER_KEY = "notcher.weather.v2";
const RATES_KEY = "notcher.rates";
/** Weather older than this is fetched again. */
const WEATHER_TTL = 15 * 60_000;
/** The provider updates rates once a day; twice a day is plenty. */
const RATES_TTL = 12 * 3600_000;

function readCache<T>(key: string): T | null {
  try {
    const raw = localStorage.getItem(key);
    return raw ? (JSON.parse(raw) as T) : null;
  } catch {
    return null;
  }
}

function writeCache(key: string, value: unknown) {
  try {
    localStorage.setItem(key, JSON.stringify(value));
  } catch {
    /* not cached */
  }
}

const message = (e: unknown) =>
  e instanceof Error && !/fetch|network|abort|timeout|HTTP/i.test(e.message) ? e.message : "Can't reach the weather service";

/** The place the user picked, or null for automatic. */
type Picked = Place | null;
const keyOf = (p: Picked) => (p ? `${p.lat.toFixed(3)},${p.lon.toFixed(3)}` : "auto");

/** Current weather, the next hours and the week, refreshed while something shows it. */
class WeatherStore {
  now = $state<Weather | null>(null);
  error = $state<string | null>(null);
  loading = $state(false);
  picked = $state<Picked>(null);
  unit = $state<TempUnit>("c");
  clock24h = $state(false);
  private ready: Promise<void> | null = null;
  private users = 0;
  private timer: ReturnType<typeof setInterval> | null = null;
  /** Which place the shown weather is for. */
  private shownFor: string | null = null;

  private init() {
    this.ready ??= (async () => {
      const s = await loadSettings();
      this.unit = s.tempUnit;
      this.clock24h = s.clock24h;
      this.picked = s.weatherCity && (s.weatherLat || s.weatherLon)
        ? { name: s.weatherCity, detail: "", lat: s.weatherLat, lon: s.weatherLon }
        : null;
      const cached = readCache<{ key: string; weather: Weather }>(WEATHER_KEY);
      if (cached && cached.key === keyOf(this.picked)) {
        this.now = cached.weather;
        this.shownFor = cached.key;
      }
    })();
    return this.ready;
  }

  async refresh(force = false) {
    await this.init();
    if (this.loading) return;
    const key = keyOf(this.picked);
    const fresh = this.now && this.shownFor === key && Date.now() - this.now.at < WEATHER_TTL;
    if (fresh && !force) return;
    if (!inTauri) {
      const p = this.picked;
      this.now = { ...PREVIEW, place: p?.name ?? PREVIEW.place, detail: p?.detail ?? PREVIEW.detail, at: Date.now() };
      this.shownFor = key;
      return;
    }
    this.loading = true;
    const picked = this.picked;
    try {
      let place: Place;
      let approximate = false;
      if (picked) place = picked;
      else ({ place, approximate } = await locateAutomatically(await preciseLocation()));
      const weather = await fetchWeather(place, approximate);
      if (key !== keyOf(this.picked)) return;
      this.now = weather;
      this.shownFor = key;
      this.error = null;
      writeCache(WEATHER_KEY, { key, weather });
    } catch (e) {
      if (key === keyOf(this.picked)) this.error = message(e);
    } finally {
      this.loading = false;
    }
  }

  search(query: string): Promise<Place[]> {
    if (!inTauri) {
      const q = query.trim().toLowerCase();
      return Promise.resolve(PREVIEW_PLACES.filter((p) => `${p.name} ${p.detail}`.toLowerCase().includes(q)));
    }
    return searchPlaces(query);
  }

  /** Picks a place, or null to find it automatically, and fetches its weather. */
  async choose(place: Picked) {
    await this.init();
    if (keyOf(place) === keyOf(this.picked)) return;
    this.picked = place;
    this.error = null;
    this.now = null;
    await saveSetting("weatherCity", place?.name ?? "");
    await saveSetting("weatherLat", place?.lat ?? 0);
    await saveSetting("weatherLon", place?.lon ?? 0);
    void notifySettingsChanged();
    await this.refresh(true);
  }

  async setUnit(unit: TempUnit) {
    await this.init();
    this.unit = unit;
    await saveSetting("tempUnit", unit);
    void notifySettingsChanged();
  }

  /** Keeps the weather fresh while a view shows it; returns a cleanup function. */
  watch(): () => void {
    this.users += 1;
    void this.refresh();
    this.timer ??= setInterval(() => void this.refresh(), 5 * 60_000);
    return () => {
      this.users -= 1;
      if (this.users === 0 && this.timer) {
        clearInterval(this.timer);
        this.timer = null;
      }
    };
  }
}

/** Exchange rates for the currency converter. */
class RatesStore {
  current = $state<Rates | null>(readCache<Rates>(RATES_KEY));
  error = $state<string | null>(null);
  private loading = false;

  async ensure() {
    if (this.loading || (this.current && Date.now() - this.current.at < RATES_TTL)) return;
    if (!inTauri) {
      this.current = { rates: PREVIEW_RATES, updated: Date.now(), at: Date.now() };
      return;
    }
    this.loading = true;
    try {
      this.current = await fetchRates();
      this.error = null;
      writeCache(RATES_KEY, this.current);
    } catch {
      this.error = this.current ? "Using saved rates (offline)" : "Can't reach the exchange-rate service";
    } finally {
      this.loading = false;
    }
  }
}

/** Browser preview data. */
const PREVIEW_HOURS = [29, 29, 28, 27, 26, 26, 25, 25, 24, 24, 24, 25];
const PREVIEW: Weather = {
  place: "Salt Lake",
  detail: "Kolkata, West Bengal, 700091",
  approximate: false,
  temp: 28.4,
  feels: 32.1,
  humidity: 74,
  wind: 11,
  code: 2,
  isDay: true,
  uv: 6,
  sunrise: "05:43",
  sunset: "17:01",
  hours: PREVIEW_HOURS.map((temp, i) => ({
    time: `2026-10-31T${String((15 + i) % 24).padStart(2, "0")}:00`,
    temp,
    code: [2, 2, 3, 61, 61, 3, 1, 0, 0, 0, 1, 2][i],
    isDay: 15 + i < 17,
    rain: [10, 10, 30, 60, 55, 20, 5, 0, 0, 0, 0, 5][i],
  })),
  days: ["2026-10-31", "2026-11-01", "2026-11-02", "2026-11-03", "2026-11-04", "2026-11-05", "2026-11-06"].map((date, i) => ({
    date,
    code: [2, 61, 3, 0, 80, 1, 2][i],
    max: [31, 29, 30, 32, 30, 31, 31][i],
    min: [24, 23, 23, 24, 22, 23, 24][i],
    rain: [40, 80, 20, 0, 60, 10, 10][i],
  })),
  at: 0,
};

const PREVIEW_PLACES: Place[] = [
  { name: "Salt Lake", detail: "Bidhannagar, West Bengal, 700091", lat: 22.58, lon: 88.41 },
  { name: "Kolkata", detail: "West Bengal, India", lat: 22.57, lon: 88.36 },
  { name: "Pune", detail: "Maharashtra, India", lat: 18.52, lon: 73.86 },
  { name: "Delhi", detail: "India", lat: 28.65, lon: 77.23 },
];

const PREVIEW_RATES: Record<string, number> = {
  USD: 1, EUR: 0.92, GBP: 0.79, INR: 84.1, JPY: 151.6, CNY: 7.12, AUD: 1.52, CAD: 1.39, CHF: 0.86, SGD: 1.32, AED: 3.67, BDT: 119.5,
};

export const weather = new WeatherStore();
export const rates = new RatesStore();
