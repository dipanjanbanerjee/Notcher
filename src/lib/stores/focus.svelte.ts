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
  /** Minutes of PC use between water reminders; 0 turns them off (set from settings). */
  waterEveryMin = $state(0);
  /** Time the PC has been in use since the last water reminder. */
  private sinceWaterMs = 0;
  private lastUpdate = Date.now();

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
    if (minutes !== this.waterEveryMin) this.sinceWaterMs = 0;
    this.waterEveryMin = minutes;
  }

  /**
   * Called about once a second. `active` is false while nobody is using the PC, so
   * time away does not count toward the water reminder. Returns what just happened,
   * for live activities.
   */
  update(now: number, active = true): { finished?: Segment | "countdown"; water?: boolean } {
    this.now = now;
    const result = tick(this.state, now);
    if (result.state !== this.state) this.set(result.state);
    // Capped, so a sleeping PC or a stalled timer never counts as use.
    const step = Math.min(5_000, Math.max(0, now - this.lastUpdate));
    this.lastUpdate = now;
    let water = false;
    if (this.waterEveryMin > 0 && active) {
      this.sinceWaterMs += step;
      if (this.sinceWaterMs >= this.waterEveryMin * 60_000) {
        this.sinceWaterMs = 0;
        water = true;
      }
    }
    return { finished: result.finished, water };
  }
}

export const focus = new FocusStore();
