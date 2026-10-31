/**
 * Picks an accent colour from cover art for the island's Glow.
 */

export type RGB = [number, number, number];

export function rgbToHsl([r, g, b]: RGB): [number, number, number] {
  r /= 255;
  g /= 255;
  b /= 255;
  const max = Math.max(r, g, b);
  const min = Math.min(r, g, b);
  const l = (max + min) / 2;
  if (max === min) return [0, 0, l];
  const d = max - min;
  const s = l > 0.5 ? d / (2 - max - min) : d / (max + min);
  let h: number;
  if (max === r) h = (g - b) / d + (g < b ? 6 : 0);
  else if (max === g) h = (b - r) / d + 2;
  else h = (r - g) / d + 4;
  return [h * 60, s, l];
}

export function hslToRgb(h: number, s: number, l: number): RGB {
  const k = (n: number) => (n + h / 30) % 12;
  const a = s * Math.min(l, 1 - l);
  const f = (n: number) => l - a * Math.max(-1, Math.min(k(n) - 3, Math.min(9 - k(n), 1)));
  return [Math.round(f(0) * 255), Math.round(f(8) * 255), Math.round(f(4) * 255)];
}

const BINS = 24;

/**
 * Finds the most prominent *vivid* colour in RGBA pixel data and lifts it to a
 * brightness that glows well on a dark island. Returns null for grey-ish images.
 */
export function vibrantColor(pixels: Uint8ClampedArray | number[], stride = 4): RGB | null {
  const weight = new Float64Array(BINS);
  const sums = Array.from({ length: BINS }, () => [0, 0, 0]);
  for (let i = 0; i + 2 < pixels.length; i += stride) {
    if (stride === 4 && pixels[i + 3] < 128) continue;
    const rgb: RGB = [pixels[i], pixels[i + 1], pixels[i + 2]];
    const [h, s, l] = rgbToHsl(rgb);
    if (s < 0.25 || l < 0.12 || l > 0.92) continue;
    // Favour saturated mid-tones.
    const w = s * (1 - Math.abs(l - 0.5) * 1.4);
    const bin = Math.floor((h / 360) * BINS) % BINS;
    weight[bin] += w;
    sums[bin][0] += rgb[0] * w;
    sums[bin][1] += rgb[1] * w;
    sums[bin][2] += rgb[2] * w;
  }
  let best = -1;
  for (let i = 0; i < BINS; i++) if (weight[i] > 0 && (best < 0 || weight[i] > weight[best])) best = i;
  if (best < 0) return null;
  const avg: RGB = [
    sums[best][0] / weight[best],
    sums[best][1] / weight[best],
    sums[best][2] / weight[best],
  ];
  const [h, s, l] = rgbToHsl(avg);
  return hslToRgb(h, Math.max(s, 0.6), Math.min(Math.max(l, 0.55), 0.68));
}

export const toCss = ([r, g, b]: RGB, alpha = 1) => `rgb(${r} ${g} ${b} / ${alpha})`;

/** Loads an image and samples it on a small canvas. Resolves null if it cannot be read. */
export async function colorFromImage(src: string): Promise<RGB | null> {
  try {
    const img = new Image();
    img.crossOrigin = "anonymous";
    img.decoding = "async";
    img.src = src;
    await img.decode();
    const size = 32;
    const canvas = document.createElement("canvas");
    canvas.width = size;
    canvas.height = size;
    const ctx = canvas.getContext("2d", { willReadFrequently: true });
    if (!ctx) return null;
    ctx.drawImage(img, 0, 0, size, size);
    return vibrantColor(ctx.getImageData(0, 0, size, size).data);
  } catch {
    // Remote art without CORS headers taints the canvas; fall back to the theme accent.
    return null;
  }
}
