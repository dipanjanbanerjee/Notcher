/**
 * Unit conversion for the Tools tab. Every unit is defined by how many of the
 * category's base unit it is; temperature needs offsets, so it has its own formulas.
 */

export type Category = "length" | "weight" | "temperature" | "volume" | "area" | "speed" | "data" | "time" | "currency";

export interface Unit {
  id: string;
  label: string;
  /** Size in the category's base unit (unused for temperature and currency). */
  factor: number;
}

export const CATEGORIES: { id: Category; label: string }[] = [
  { id: "length", label: "Length" },
  { id: "weight", label: "Weight" },
  { id: "temperature", label: "Temperature" },
  { id: "volume", label: "Volume" },
  { id: "area", label: "Area" },
  { id: "speed", label: "Speed" },
  { id: "data", label: "Data" },
  { id: "time", label: "Time" },
  { id: "currency", label: "Currency" },
];

const u = (id: string, label: string, factor: number): Unit => ({ id, label, factor });

/** Base units: metre, kilogram, litre, square metre, metre per second, byte, second. */
export const UNITS: Record<Exclude<Category, "currency">, Unit[]> = {
  length: [
    u("mm", "Millimetres", 0.001),
    u("cm", "Centimetres", 0.01),
    u("m", "Metres", 1),
    u("km", "Kilometres", 1000),
    u("in", "Inches", 0.0254),
    u("ft", "Feet", 0.3048),
    u("yd", "Yards", 0.9144),
    u("mi", "Miles", 1609.344),
  ],
  weight: [
    u("mg", "Milligrams", 1e-6),
    u("g", "Grams", 0.001),
    u("kg", "Kilograms", 1),
    u("t", "Tonnes", 1000),
    u("oz", "Ounces", 0.028349523125),
    u("lb", "Pounds", 0.45359237),
    u("st", "Stones", 6.35029318),
  ],
  temperature: [u("c", "Celsius", 1), u("f", "Fahrenheit", 1), u("k", "Kelvin", 1)],
  volume: [
    u("ml", "Millilitres", 0.001),
    u("l", "Litres", 1),
    u("m3", "Cubic metres", 1000),
    u("tsp", "Teaspoons (US)", 0.00492892159375),
    u("tbsp", "Tablespoons (US)", 0.01478676478125),
    u("cup", "Cups (US)", 0.2365882365),
    u("floz", "Fluid ounces (US)", 0.0295735295625),
    u("gal", "Gallons (US)", 3.785411784),
  ],
  area: [
    u("cm2", "Square centimetres", 1e-4),
    u("m2", "Square metres", 1),
    u("ha", "Hectares", 10_000),
    u("km2", "Square kilometres", 1e6),
    u("ft2", "Square feet", 0.09290304),
    u("ac", "Acres", 4046.8564224),
    u("mi2", "Square miles", 2_589_988.110336),
  ],
  speed: [
    u("mps", "Metres per second", 1),
    u("kmh", "Kilometres per hour", 1 / 3.6),
    u("mph", "Miles per hour", 0.44704),
    u("kn", "Knots", 1852 / 3600),
  ],
  data: [
    u("b", "Bytes", 1),
    u("kb", "Kilobytes", 1024),
    u("mb", "Megabytes", 1024 ** 2),
    u("gb", "Gigabytes", 1024 ** 3),
    u("tb", "Terabytes", 1024 ** 4),
    u("bit", "Bits", 1 / 8),
    u("mbit", "Megabits", 1e6 / 8),
  ],
  time: [
    u("ms", "Milliseconds", 0.001),
    u("s", "Seconds", 1),
    u("min", "Minutes", 60),
    u("h", "Hours", 3600),
    u("d", "Days", 86_400),
    u("wk", "Weeks", 604_800),
    u("yr", "Years", 31_557_600),
  ],
};

/** The pair a category starts with. */
export const DEFAULT_PAIR: Record<Category, [string, string]> = {
  length: ["km", "mi"],
  weight: ["kg", "lb"],
  temperature: ["c", "f"],
  volume: ["l", "gal"],
  area: ["m2", "ft2"],
  speed: ["kmh", "mph"],
  data: ["gb", "mb"],
  time: ["h", "min"],
  currency: ["USD", "INR"],
};

function toCelsius(v: number, from: string): number {
  if (from === "f") return ((v - 32) * 5) / 9;
  if (from === "k") return v - 273.15;
  return v;
}

function fromCelsius(c: number, to: string): number {
  if (to === "f") return (c * 9) / 5 + 32;
  if (to === "k") return c + 273.15;
  return c;
}

/**
 * Converts `value` between two units of a category. Currency uses `rates` (units of
 * each currency per one US dollar). Returns null when a unit or rate is unknown.
 */
export function convert(
  category: Category,
  value: number,
  from: string,
  to: string,
  rates: Record<string, number> = {},
): number | null {
  if (!Number.isFinite(value)) return null;
  if (category === "currency") {
    const a = rates[from];
    const b = rates[to];
    return a && b ? (value / a) * b : null;
  }
  if (category === "temperature") return fromCelsius(toCelsius(value, from), to);
  const list = UNITS[category];
  const a = list.find((x) => x.id === from);
  const b = list.find((x) => x.id === to);
  return a && b ? (value * a.factor) / b.factor : null;
}

/** A tidy number for display: up to 6 significant digits, no trailing zeros. */
export function formatNumber(n: number | null): string {
  if (n === null || !Number.isFinite(n)) return "—";
  if (n === 0) return "0";
  const abs = Math.abs(n);
  if (abs >= 1e12 || abs < 1e-6) return n.toExponential(3).replace(/\.?0+e/, "e");
  const digits = abs >= 1 ? Math.max(0, 6 - Math.floor(Math.log10(abs)) - 1) : 6;
  return Number(n.toFixed(Math.min(10, digits))).toLocaleString("en-US", { maximumFractionDigits: 10 });
}

/** Currencies offered first; any other code the rates include works too. */
export const POPULAR_CURRENCIES = ["USD", "EUR", "GBP", "INR", "JPY", "CNY", "AUD", "CAD", "CHF", "SGD", "AED", "BDT"];

const SYMBOLS: Record<string, string> = {
  kmh: "km/h", mps: "m/s", m2: "m²", cm2: "cm²", km2: "km²", ft2: "ft²", mi2: "mi²", m3: "m³",
  c: "°C", f: "°F", k: "K", floz: "fl oz", b: "B", kb: "KB", mb: "MB", gb: "GB", tb: "TB", mbit: "Mbit",
};

/** A short symbol for a unit id ("km/h", "m²"); currencies are already short. */
export function symbol(id: string): string {
  return SYMBOLS[id] ?? id;
}
