/**
 * A tiny spring-physics engine for the island's shape.
 *
 * Springs are interruptible by nature: changing the target mid-flight keeps the
 * current position and velocity, so the motion bends smoothly toward the new goal
 * instead of restarting.
 */

export interface SpringConfig {
  /** Higher is snappier. */
  stiffness: number;
  /** Higher settles with less overshoot. */
  damping: number;
  mass: number;
}

export const SPRINGS = {
  /** Default shape motion: lively with a small, soft overshoot. */
  shape: { stiffness: 380, damping: 30, mass: 1 },
  /** Closing: a touch firmer so it tucks away without wobble. */
  close: { stiffness: 420, damping: 38, mass: 1 },
  /** Gentle motion used for small ambient changes. */
  soft: { stiffness: 170, damping: 26, mass: 1 },
} satisfies Record<string, SpringConfig>;

/** Fixed physics step. Small steps keep stiff springs stable at any frame rate. */
const STEP = 1 / 240;
/** Never simulate more than this many seconds in one frame (e.g. after a hidden tab). */
const MAX_FRAME = 0.064;
const REST_DELTA = 0.01;
const REST_VELOCITY = 0.01;

export class Spring {
  value: number;
  velocity = 0;
  target: number;
  config: SpringConfig;

  constructor(value: number, config: SpringConfig = SPRINGS.shape) {
    this.value = value;
    this.target = value;
    this.config = config;
  }

  get settled(): boolean {
    return (
      Math.abs(this.target - this.value) < REST_DELTA &&
      Math.abs(this.velocity) < REST_VELOCITY
    );
  }

  /** Advances the simulation by `dt` seconds. Returns true once at rest. */
  step(dt: number): boolean {
    let remaining = Math.min(Math.max(dt, 0), MAX_FRAME);
    const { stiffness, damping, mass } = this.config;
    while (remaining > 0) {
      const h = Math.min(STEP, remaining);
      // Semi-implicit Euler: stable for the stiffness range we use.
      const force = -stiffness * (this.value - this.target) - damping * this.velocity;
      this.velocity += (force / mass) * h;
      this.value += this.velocity * h;
      remaining -= h;
    }
    if (this.settled) {
      this.value = this.target;
      this.velocity = 0;
      return true;
    }
    return false;
  }

  /** Jumps straight to a value (used for reduced motion). */
  snap(value: number) {
    this.value = value;
    this.target = value;
    this.velocity = 0;
  }
}

/** A set of named springs animated together on one animation-frame loop. */
export class SpringGroup<K extends string> {
  readonly springs: Record<K, Spring>;
  private frame = 0;
  private last = 0;
  private listener: (values: Record<K, number>, settled: boolean) => void;
  private now: () => number;
  private raf: (cb: FrameRequestCallback) => number;
  private caf: (id: number) => void;

  constructor(
    initial: Record<K, number>,
    listener: (values: Record<K, number>, settled: boolean) => void,
    env: {
      now?: () => number;
      raf?: (cb: FrameRequestCallback) => number;
      caf?: (id: number) => void;
    } = {},
  ) {
    this.springs = Object.fromEntries(
      Object.entries(initial).map(([k, v]) => [k, new Spring(v as number)]),
    ) as Record<K, Spring>;
    this.listener = listener;
    this.now = env.now ?? (() => performance.now());
    this.raf = env.raf ?? ((cb) => requestAnimationFrame(cb));
    this.caf = env.caf ?? ((id) => cancelAnimationFrame(id));
  }

  get values(): Record<K, number> {
    const out = {} as Record<K, number>;
    for (const k in this.springs) out[k] = this.springs[k].value;
    return out;
  }

  get animating(): boolean {
    return this.frame !== 0;
  }

  /** Sets new targets. With `instant`, jumps there without animation. */
  to(targets: Partial<Record<K, number>>, config?: SpringConfig, instant = false) {
    for (const k in targets) {
      const spring = this.springs[k];
      const target = targets[k];
      if (!spring || target === undefined) continue;
      if (config) spring.config = config;
      if (instant) spring.snap(target);
      else spring.target = target;
    }
    if (instant) {
      this.stop();
      this.listener(this.values, true);
      return;
    }
    if (!this.frame) {
      this.last = this.now();
      this.frame = this.raf(this.tick);
    }
  }

  stop() {
    if (this.frame) this.caf(this.frame);
    this.frame = 0;
  }

  private tick = () => {
    const t = this.now();
    const dt = (t - this.last) / 1000;
    this.last = t;
    let done = true;
    for (const k in this.springs) {
      if (!this.springs[k].step(dt)) done = false;
    }
    this.frame = done ? 0 : this.raf(this.tick);
    this.listener(this.values, done);
  };
}
