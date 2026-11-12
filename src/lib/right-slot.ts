/**
 * What the right side of the resting notch shows: the clock, or the temperature and
 * air quality from the shared weather store. Weather falls back to the clock until it
 * has loaded, and again when it is too old to trust (offline for hours).
 */

import type { NotchRightSlot } from "./settings";
import { formatClock, meridiem } from "./time";
import { formatTemp, type TempUnit, type Weather } from "./weather";

/** Weather older than this is no longer shown in the notch; the clock comes back. */
export const WEATHER_STALE_MS = 3 * 3600_000;

export interface RightSlot {
  kind: "clock" | "weather";
  /** Bold, bright text: the time or the temperature. */
  primary: string;
  /** Muted text: AM/PM (empty on a 24-hour clock) or the AQI. */
  secondary: string;
}

/** True when this weather can stand in for the clock. */
export function weatherUsable(w: Weather | null | undefined, now: number): w is Weather {
  return !!w && Number.isFinite(w.temp) && now - w.at < WEATHER_STALE_MS;
}

export function rightSlot(
  choice: NotchRightSlot,
  clock: Date,
  clock24h: boolean,
  weather: Weather | null | undefined,
  unit: TempUnit,
): RightSlot {
  if (choice === "weather_aqi" && weatherUsable(weather, clock.getTime())) {
    const aqi = weather.aqi;
    return {
      kind: "weather",
      primary: `${formatTemp(weather.temp, unit)}${unit.toUpperCase()}`,
      secondary: typeof aqi === "number" ? `AQI ${aqi}` : "AQI –",
    };
  }
  return { kind: "clock", primary: formatClock(clock, clock24h, true), secondary: clock24h ? "" : meridiem(clock) };
}
