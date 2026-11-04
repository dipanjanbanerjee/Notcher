/**
 * A short, soft two-note chime made with Web Audio, so no sound file is needed.
 */

let context: AudioContext | null = null;

/** Plays the chime. Safe to call anywhere; it does nothing if audio is unavailable. */
export async function playChime(volume = 0.18): Promise<void> {
  try {
    context ??= new AudioContext();
    if (context.state === "suspended") await context.resume();
    const start = context.currentTime + 0.02;
    note(context, 880, start, 0.16, volume);
    note(context, 1318.5, start + 0.12, 0.26, volume);
  } catch {
    /* no audio device, or audio is blocked */
  }
}

function note(ctx: AudioContext, frequency: number, at: number, length: number, volume: number) {
  const osc = ctx.createOscillator();
  const gain = ctx.createGain();
  osc.type = "sine";
  osc.frequency.value = frequency;
  // Quick rise and a smooth fade, so it sounds like a bell rather than a buzz.
  gain.gain.setValueAtTime(0, at);
  gain.gain.linearRampToValueAtTime(volume, at + 0.012);
  gain.gain.exponentialRampToValueAtTime(0.0001, at + length);
  osc.connect(gain).connect(ctx.destination);
  osc.start(at);
  osc.stop(at + length + 0.02);
}
