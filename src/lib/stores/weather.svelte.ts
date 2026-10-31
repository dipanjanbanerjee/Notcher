import { inTauri, notifySettingsChanged } from "../bridge";
import { loadSettings, saveSetting } from "../settings";
import { fetchRates, fetchWeather, locate, type Rates, type TempUnit, type Weather } from "../weather";

const WEATHER_KEY = "notcher.weather";
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

const message = (e: unknown) => (e instanceof Error && !/fetch|network|abort|timeout/i.test(e.message) ? e.message : "Can't reach the weather service");

/** Current weather and a 5-day outlook, refreshed while something on screen shows it. */
class WeatherStore {
  now = $state<Weather | null>(null);
  error = $state<string | null>(null);
  loading = $state(false);
  city = $state("");
  unit = $state<TempUnit>("c");
  private ready: Promise<void> | null = null;
  private users = 0;
  private timer: ReturnType<typeof setInterval> | null = null;
  /** The city the cached weather is for. */
  private cachedFor: string | null = null;

  private init() {
    this.ready ??= (async () => {
      const s = await loadSettings();
      this.city = s.weatherCity;
      this.unit = s.tempUnit;
      const cached = readCache<{ city: string; weather: Weather }>(WEATHER_KEY);
      if (cached && cached.city === this.city) {
        this.now = cached.weather;
        this.cachedFor = cached.city;
      }
    })();
    return this.ready;
  }

  async refresh(force = false) {
    await this.init();
    if (this.loading) return;
    const fresh = this.now && this.cachedFor === this.city && Date.now() - this.now.at < WEATHER_TTL;
    if (fresh && !force) return;
    if (!inTauri) {
      this.now = { ...PREVIEW, place: this.city || PREVIEW.place, at: Date.now() };
      this.cachedFor = this.city;
      return;
    }
    this.loading = true;
    const city = this.city;
    try {
      const weather = await fetchWeather(await locate(city));
      if (city !== this.city) return;
      this.now = weather;
      this.cachedFor = city;
      this.error = null;
      writeCache(WEATHER_KEY, { city, weather });
    } catch (e) {
      if (city === this.city) this.error = message(e);
    } finally {
      this.loading = false;
    }
  }

  /** Changes the city ("" for automatic) and fetches its weather. */
  async setCity(city: string) {
    await this.init();
    const next = city.trim().slice(0, 80);
    if (next === this.city) return;
    this.city = next;
    this.now = null;
    this.error = null;
    await saveSetting("weatherCity", next);
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
const PREVIEW: Weather = {
  place: "Kolkata",
  temp: 28.4,
  feels: 32.1,
  humidity: 74,
  wind: 11,
  code: 2,
  isDay: true,
  days: ["2026-10-31", "2026-11-01", "2026-11-02", "2026-11-03", "2026-11-04"].map((date, i) => ({
    date,
    code: [2, 61, 3, 0, 80][i],
    max: [31, 29, 30, 32, 30][i],
    min: [24, 23, 23, 24, 22][i],
  })),
  at: 0,
};

const PREVIEW_RATES: Record<string, number> = {
  USD: 1, EUR: 0.92, GBP: 0.79, INR: 84.1, JPY: 151.6, CNY: 7.12, AUD: 1.52, CAD: 1.39, CHF: 0.86, SGD: 1.32, AED: 3.67, BDT: 119.5,
};

export const weather = new WeatherStore();
export const rates = new RatesStore();
