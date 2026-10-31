import { describe, expect, it } from "vitest";
import { BeatFollower } from "./beat";

function run(follower: BeatFollower, levels: number[], frameMs = 16) {
  let t = 0;
  const out: number[] = [];
  for (const l of levels) {
    follower.input(l);
    for (let i = 0; i < 2; i++) {
      t += frameMs;
      out.push(follower.step(frameMs, t));
    }
  }
  return out;
}

describe("beat follower", () => {
  it("stays dark in silence", () => {
    const f = new BeatFollower();
    expect(Math.max(...run(f, Array(60).fill(0)))).toBe(0);
    expect(f.quiet).toBe(true);
  });

  it("kicks harder on a beat than on a steady level", () => {
    const steady = new BeatFollower();
    const steadyOut = run(steady, Array(120).fill(0.3));
    const settled = steadyOut[steadyOut.length - 1];

    const beat = new BeatFollower();
    const beatOut = run(beat, [...Array(120).fill(0.3), 0.9, 0.9]);
    expect(Math.max(...beatOut.slice(-4))).toBeGreaterThan(settled + 0.2);
  });

  it("fades out after the music stops", () => {
    const f = new BeatFollower();
    run(f, Array(30).fill(0.8));
    run(f, Array(200).fill(0));
    expect(f.quiet).toBe(true);
  });

  it("ignores bad input", () => {
    const f = new BeatFollower();
    f.input(Number.NaN);
    expect(f.step(16, 16)).toBe(0);
  });
});
