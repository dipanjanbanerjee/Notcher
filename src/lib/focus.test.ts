import { describe, expect, it } from "vitest";
import {
  DEFAULT_FOCUS_CONFIG as C,
  display,
  initialState,
  pause,
  progress,
  reset,
  restore,
  setCountdown,
  start,
  tick,
} from "./focus";

const MIN = 60_000;

describe("focus timers", () => {
  it("counts a Pomodoro down and pauses/resumes exactly", () => {
    let s = start(initialState("pomodoro"), 0);
    expect(display(s, 5 * MIN)).toBe(20 * MIN);
    s = pause(s, 5 * MIN);
    expect(display(s, 50 * MIN)).toBe(20 * MIN);
    s = start(s, 50 * MIN);
    expect(display(s, 51 * MIN)).toBe(19 * MIN);
    expect(progress(s, 51 * MIN)).toBeCloseTo(6 / 25);
  });

  it("rolls focus into a break automatically, then waits after the break", () => {
    let s = start(initialState("pomodoro"), 0);
    let r = tick(s, 25 * MIN);
    expect(r.finished).toBe("focus");
    expect(r.state.segment).toBe("short-break");
    expect(r.state.phase).toBe("running");
    expect(display(r.state, 26 * MIN)).toBe(4 * MIN);
    r = tick(r.state, 30 * MIN);
    expect(r.finished).toBe("short-break");
    expect(r.state.segment).toBe("focus");
    expect(r.state.phase).toBe("idle");
    s = r.state;
    expect(s.completed).toBe(1);
  });

  it("gives a long break after four sessions", () => {
    let s = { ...initialState("pomodoro"), completed: 3 };
    s = start(s, 0);
    const r = tick(s, 25 * MIN);
    expect(r.state.segment).toBe("long-break");
    expect(r.state.durationMs).toBe(C.longBreakMin * MIN);
  });

  it("does nothing before the end", () => {
    const s = start(initialState("pomodoro"), 0);
    expect(tick(s, 10 * MIN).finished).toBeUndefined();
  });

  it("runs a custom countdown and keeps its length on reset", () => {
    let s = start(setCountdown(initialState(), 3), 0);
    const r = tick(s, 3 * MIN);
    expect(r.finished).toBe("countdown");
    s = reset(r.state);
    expect(s.durationMs).toBe(3 * MIN);
    expect(setCountdown(s, 0).durationMs).toBe(MIN);
  });

  it("runs a stopwatch with pause", () => {
    let s = start(initialState("stopwatch"), 1000);
    expect(display(s, 4000)).toBe(3000);
    s = pause(s, 4000);
    s = start(s, 10_000);
    expect(display(s, 11_000)).toBe(4000);
  });

  it("rejects corrupt saved state", () => {
    expect(restore(null)).toBeNull();
    expect(restore({ kind: "egg" })).toBeNull();
    expect(restore({ ...initialState(), anchor: NaN })).toBeNull();
    expect(restore(initialState())).toEqual(initialState());
  });
});
