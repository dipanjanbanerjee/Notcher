import { describe, expect, it } from "vitest";
import { SPRINGS, Spring, SpringGroup } from "./spring";

describe("Spring", () => {
  it("settles exactly on its target", () => {
    const s = new Spring(0, SPRINGS.shape);
    s.target = 100;
    let frames = 0;
    while (!s.step(1 / 60) && frames < 600) frames++;
    expect(s.value).toBe(100);
    expect(s.velocity).toBe(0);
    expect(frames).toBeLessThan(90); // under ~1.5 s
  });

  it("overshoots only slightly with the shape spring", () => {
    const s = new Spring(0, SPRINGS.shape);
    s.target = 100;
    let max = 0;
    for (let i = 0; i < 240; i++) {
      s.step(1 / 60);
      max = Math.max(max, s.value);
    }
    expect(max).toBeGreaterThan(100);
    expect(max).toBeLessThan(110);
  });

  it("behaves the same at 60 Hz and 144 Hz", () => {
    const a = new Spring(0);
    const b = new Spring(0);
    a.target = b.target = 50;
    for (let i = 0; i < 30; i++) a.step(1 / 60);
    for (let i = 0; i < 72; i++) b.step(1 / 144);
    expect(Math.abs(a.value - b.value)).toBeLessThan(0.05);
  });

  it("keeps velocity when retargeted mid-flight", () => {
    const s = new Spring(0);
    s.target = 100;
    for (let i = 0; i < 5; i++) s.step(1 / 60);
    const v = s.velocity;
    s.target = 0;
    expect(s.velocity).toBe(v);
  });

  it("ignores huge frame gaps instead of exploding", () => {
    const s = new Spring(0);
    s.target = 10;
    s.step(30);
    expect(Number.isFinite(s.value)).toBe(true);
    expect(Math.abs(s.value)).toBeLessThan(20);
  });
});

describe("SpringGroup", () => {
  function fakeClock() {
    let t = 0;
    const queue: FrameRequestCallback[] = [];
    return {
      env: {
        now: () => t,
        raf: (cb: FrameRequestCallback) => queue.push(cb),
        caf: () => {},
      },
      runFrames(n: number) {
        for (let i = 0; i < n && queue.length; i++) {
          t += 1000 / 60;
          queue.shift()!(t);
        }
      },
    };
  }

  it("animates all values to their targets and stops", () => {
    const clock = fakeClock();
    const seen: number[] = [];
    const g = new SpringGroup({ w: 10, h: 10 }, (v) => seen.push(v.w), clock.env);
    g.to({ w: 200, h: 50 });
    clock.runFrames(600);
    expect(g.values).toEqual({ w: 200, h: 50 });
    expect(g.animating).toBe(false);
    expect(seen.length).toBeGreaterThan(10);
  });

  it("jumps immediately when instant", () => {
    const clock = fakeClock();
    let last = { w: 0 };
    const g = new SpringGroup({ w: 0 }, (v) => (last = v), clock.env);
    g.to({ w: 42 }, undefined, true);
    expect(last.w).toBe(42);
    expect(g.animating).toBe(false);
  });
});
