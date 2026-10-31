/**
 * Focus timers: Pomodoro, countdown and stopwatch. Pure functions over a plain
 * state object, so the timer survives restarts (it is stored as timestamps) and is
 * easy to test.
 */

export type FocusKind = "pomodoro" | "countdown" | "stopwatch";
export type Phase = "idle" | "running" | "paused";
/** Within a Pomodoro cycle. */
export type Segment = "focus" | "short-break" | "long-break";

export interface FocusConfig {
  focusMin: number;
  shortBreakMin: number;
  longBreakMin: number;
  /** A long break comes after this many focus sessions. */
  sessionsPerLongBreak: number;
}

export const DEFAULT_FOCUS_CONFIG: FocusConfig = {
  focusMin: 25,
  shortBreakMin: 5,
  longBreakMin: 15,
  sessionsPerLongBreak: 4,
};

export interface FocusState {
  kind: FocusKind;
  phase: Phase;
  segment: Segment;
  /** Length of the current countdown segment. */
  durationMs: number;
  /** When running: the moment the countdown ends (countdown) or the stopwatch's virtual start. */
  anchor: number;
  /** When paused: time left (countdown) or time elapsed (stopwatch). */
  frozenMs: number;
  /** Focus sessions finished in this Pomodoro cycle. */
  completed: number;
}

export function initialState(kind: FocusKind = "pomodoro", config = DEFAULT_FOCUS_CONFIG): FocusState {
  return {
    kind,
    phase: "idle",
    segment: "focus",
    durationMs: kind === "pomodoro" ? config.focusMin * 60_000 : kind === "countdown" ? 10 * 60_000 : 0,
    anchor: 0,
    frozenMs: 0,
    completed: 0,
  };
}

/** Milliseconds left on a countdown, or elapsed on a stopwatch. */
export function display(state: FocusState, now: number): number {
  if (state.kind === "stopwatch") {
    if (state.phase === "running") return Math.max(0, now - state.anchor);
    return state.frozenMs;
  }
  if (state.phase === "running") return Math.max(0, state.anchor - now);
  if (state.phase === "paused") return state.frozenMs;
  return state.durationMs;
}

/** Fraction of the current segment done, 0..1 (stopwatch: position within the minute). */
export function progress(state: FocusState, now: number): number {
  if (state.kind === "stopwatch") return (display(state, now) % 60_000) / 60_000;
  if (state.durationMs <= 0) return 0;
  return 1 - display(state, now) / state.durationMs;
}

export function start(state: FocusState, now: number): FocusState {
  if (state.phase === "running") return state;
  if (state.kind === "stopwatch") {
    return { ...state, phase: "running", anchor: now - state.frozenMs };
  }
  const left = state.phase === "paused" ? state.frozenMs : state.durationMs;
  return { ...state, phase: "running", anchor: now + left };
}

export function pause(state: FocusState, now: number): FocusState {
  if (state.phase !== "running") return state;
  return { ...state, phase: "paused", frozenMs: display(state, now) };
}

export function reset(state: FocusState, config = DEFAULT_FOCUS_CONFIG): FocusState {
  const fresh = initialState(state.kind, config);
  // A custom countdown keeps its chosen length.
  if (state.kind === "countdown") fresh.durationMs = state.durationMs;
  return fresh;
}

export function setCountdown(state: FocusState, minutes: number): FocusState {
  const ms = Math.round(Math.min(Math.max(minutes, 1), 24 * 60) * 60_000);
  return { ...initialState("countdown"), durationMs: ms };
}

function segmentLength(segment: Segment, config: FocusConfig): number {
  const min =
    segment === "focus" ? config.focusMin : segment === "short-break" ? config.shortBreakMin : config.longBreakMin;
  return min * 60_000;
}

export interface TickResult {
  state: FocusState;
  /** Set when a countdown segment just ended. */
  finished?: Segment | "countdown";
}

/**
 * Advances the timer. When a Pomodoro focus session ends, the break starts by
 * itself; when a break ends, the next focus session waits for the user.
 */
export function tick(state: FocusState, now: number, config = DEFAULT_FOCUS_CONFIG): TickResult {
  if (state.kind === "stopwatch" || state.phase !== "running" || now < state.anchor) {
    return { state };
  }
  if (state.kind === "countdown") {
    return { state: { ...state, phase: "idle" }, finished: "countdown" };
  }
  const ended = state.segment;
  if (ended === "focus") {
    const completed = state.completed + 1;
    const segment: Segment = completed % config.sessionsPerLongBreak === 0 ? "long-break" : "short-break";
    const durationMs = segmentLength(segment, config);
    return {
      state: { ...state, segment, completed, durationMs, phase: "running", anchor: state.anchor + durationMs },
      finished: ended,
    };
  }
  const durationMs = segmentLength("focus", config);
  return {
    state: {
      ...state,
      segment: "focus",
      durationMs,
      phase: "idle",
      anchor: 0,
      completed: ended === "long-break" ? 0 : state.completed,
    },
    finished: ended,
  };
}

export function segmentLabel(state: FocusState): string {
  if (state.kind === "stopwatch") return "Stopwatch";
  if (state.kind === "countdown") return "Timer";
  return state.segment === "focus" ? "Focus" : state.segment === "short-break" ? "Short break" : "Long break";
}

/** Validates state loaded from storage. */
export function restore(raw: unknown): FocusState | null {
  if (!raw || typeof raw !== "object") return null;
  const s = raw as Partial<FocusState>;
  const kinds: FocusKind[] = ["pomodoro", "countdown", "stopwatch"];
  const phases: Phase[] = ["idle", "running", "paused"];
  const segments: Segment[] = ["focus", "short-break", "long-break"];
  if (!kinds.includes(s.kind as FocusKind) || !phases.includes(s.phase as Phase) || !segments.includes(s.segment as Segment)) {
    return null;
  }
  for (const k of ["durationMs", "anchor", "frozenMs", "completed"] as const) {
    if (typeof s[k] !== "number" || !Number.isFinite(s[k])) return null;
  }
  return s as FocusState;
}
