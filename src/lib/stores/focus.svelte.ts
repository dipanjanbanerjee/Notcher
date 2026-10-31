import {
  display,
  initialState,
  pause,
  progress,
  reset,
  restore,
  segmentLabel,
  setCountdown,
  start,
  tick,
  type FocusKind,
  type FocusState,
  type Segment,
} from "../focus";

const STORAGE_KEY = "notcher.focus";
const WATER_KEY = "notcher.water";

function load(): FocusState {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    return (raw && restore(JSON.parse(raw))) || initialState();
  } catch {
    return initialState();
  }
}

/** Timers shared by the Focus tab, Home and the resting island. */
class FocusStore {
  state = $state<FocusState>(load());
  now = $state(Date.now());
  /** Minutes between hydration nudges; 0 turns them off. */
  waterEveryMin = $state(0);
  private lastWater = Date.now();

  constructor() {
    try {
      const w = Number(localStorage.getItem(WATER_KEY));
      if (Number.isFinite(w) && w >= 0) this.waterEveryMin = w;
    } catch {
      /* storage unavailable: keep defaults */
    }
  }

  get running() {
    return this.state.phase === "running";
  }
  get active() {
    return this.state.phase !== "idle";
  }
  get remaining() {
    return display(this.state, this.now);
  }
  get progress() {
    return progress(this.state, this.now);
  }
  get label() {
    return segmentLabel(this.state);
  }

  private set(next: FocusState) {
    this.state = next;
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(next));
    } catch {
      /* not persisted */
    }
  }

  toggle() {
    this.now = Date.now();
    this.set(this.running ? pause(this.state, this.now) : start(this.state, this.now));
  }
  reset() {
    this.set(reset(this.state));
  }
  choose(kind: FocusKind) {
    if (kind === this.state.kind) return;
    this.set(initialState(kind));
  }
  setMinutes(minutes: number) {
    this.set(setCountdown(this.state, minutes));
  }
  setWater(minutes: number) {
    this.waterEveryMin = minutes;
    this.lastWater = Date.now();
    try {
      localStorage.setItem(WATER_KEY, String(minutes));
    } catch {
      /* not persisted */
    }
  }

  /** Called every 250 ms. Returns what just happened, for live activities. */
  update(now: number): { finished?: Segment | "countdown"; water?: boolean } {
    this.now = now;
    const result = tick(this.state, now);
    if (result.state !== this.state) this.set(result.state);
    let water = false;
    if (this.waterEveryMin > 0 && now - this.lastWater >= this.waterEveryMin * 60_000) {
      this.lastWater = now;
      water = true;
    }
    return { finished: result.finished, water };
  }
}

export const focus = new FocusStore();
