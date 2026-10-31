/**
 * Turns the speaker level (0..1, about 30 readings a second) into a smooth glow
 * strength that swells with the music and kicks on each beat.
 *
 * A fast envelope follows the sound; a slow one tracks the average loudness. When
 * the fast one jumps clearly above the average, that's a beat: a pulse fires and
 * fades out over a few frames.
 */
export class BeatFollower {
  private target = 0;
  private fast = 0;
  private slow = 0;
  private pulse = 0;
  private lastBeat = -Infinity;

  input(level: number) {
    this.target = Math.min(1, Math.max(0, Number.isFinite(level) ? level : 0));
  }

  /** Advances by `dt` milliseconds and returns the glow strength, 0..1. */
  step(dt: number, now: number): number {
    const ease = (tau: number) => 1 - Math.exp(-Math.max(0, dt) / tau);
    const v = this.target;
    this.fast += (v - this.fast) * ease(v > this.fast ? 30 : 140);
    this.slow += (v - this.slow) * ease(1200);
    if (this.fast > this.slow * 1.25 + 0.05 && now - this.lastBeat > 200) {
      this.pulse = 1;
      this.lastBeat = now;
    }
    this.pulse *= Math.exp(-Math.max(0, dt) / 180);
    return Math.min(1, Math.min(1, this.fast * 1.3) * 0.55 + this.pulse * 0.45);
  }

  /** True once everything has faded out, so the animation loop can stop. */
  get quiet(): boolean {
    return this.target === 0 && this.fast < 0.005 && this.pulse < 0.005;
  }

  reset() {
    this.target = this.fast = this.slow = this.pulse = 0;
    this.lastBeat = -Infinity;
  }
}
