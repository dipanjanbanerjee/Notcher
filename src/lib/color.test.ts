import { describe, expect, it } from "vitest";
import { hslToRgb, rgbToHsl, vibrantColor } from "./color";

const pixels = (colors: [number, number, number][], repeat = 10) =>
  colors.flatMap((c) => Array.from({ length: repeat }, () => [...c, 255])).flat();

describe("color", () => {
  it("round-trips through HSL", () => {
    const [h, s, l] = rgbToHsl([200, 80, 40]);
    const back = hslToRgb(h, s, l);
    back.forEach((v, i) => expect(Math.abs(v - [200, 80, 40][i])).toBeLessThanOrEqual(1));
  });

  it("returns null for greyscale art", () => {
    expect(vibrantColor(pixels([[20, 20, 20], [128, 128, 128], [240, 240, 240]]))).toBeNull();
  });

  it("picks the dominant vivid hue and ignores grey", () => {
    const c = vibrantColor(pixels([[30, 30, 30], [220, 40, 40], [210, 50, 45], [40, 40, 200], [230, 30, 35]]));
    expect(c).not.toBeNull();
    const [h] = rgbToHsl(c!);
    expect(h < 20 || h > 340).toBe(true); // red
  });

  it("lifts dark colours so they glow", () => {
    const c = vibrantColor(pixels([[20, 60, 30]]));
    const [, , l] = rgbToHsl(c!);
    expect(l).toBeGreaterThanOrEqual(0.54);
  });
});
