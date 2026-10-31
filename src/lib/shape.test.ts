import { describe, expect, it } from "vitest";
import { inflate, islandBounds, islandPath, normalize, sameRect, snapOut, union } from "./shape";

describe("island shape", () => {
  const g = { width: 200, height: 40, radius: 14, shoulder: 10 };

  it("produces a closed path that starts and ends on the top edge", () => {
    const d = islandPath(g, 380);
    expect(d.startsWith("M 270 0 L 490 0")).toBe(true);
    expect(d.endsWith("Z")).toBe(true);
    expect(d).not.toContain("NaN");
  });

  it("clamps corners for tiny shapes", () => {
    const n = normalize({ width: 20, height: 8, radius: 50, shoulder: 50 });
    expect(n.shoulder).toBeLessThanOrEqual(4);
    expect(n.radius * 1.28).toBeLessThanOrEqual(Math.min(10, 8 - n.shoulder) + 1e-9);
    expect(islandPath({ width: 0, height: 0, radius: 0, shoulder: 0 }, 0)).not.toContain("NaN");
  });

  it("computes bounds including shoulders", () => {
    expect(islandBounds(g, 380)).toEqual({ x: 270, y: 0, width: 220, height: 40 });
  });

  it("unions, inflates and snaps rectangles", () => {
    const u = union({ x: 10, y: 0, width: 10, height: 10 }, { x: 0, y: 0, width: 5, height: 30 });
    expect(u).toEqual({ x: 0, y: 0, width: 20, height: 30 });
    expect(inflate(u, 4, 2)).toEqual({ x: -4, y: 0, width: 28, height: 32 });
    expect(snapOut({ x: 1.5, y: 0.2, width: 2.2, height: 3.1 })).toEqual({ x: 1, y: 0, width: 3, height: 4 });
    expect(sameRect(null, u)).toBe(false);
    expect(sameRect({ ...u }, u)).toBe(true);
  });
});
