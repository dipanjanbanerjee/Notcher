import { inTauri, screenTime, type ScreenTime } from "../bridge";

/** Today's screen time, refreshed while something on screen shows it. */
class ScreenTimeStore {
  today = $state<ScreenTime | null>(null);
  private users = 0;
  private timer: ReturnType<typeof setInterval> | null = null;

  async refresh() {
    if (!inTauri) {
      this.today ??= PREVIEW;
      return;
    }
    const t = await screenTime();
    if (t) this.today = t;
  }

  /** Keeps the numbers fresh while a widget is showing; returns a cleanup function. */
  watch(): () => void {
    this.users += 1;
    void this.refresh();
    this.timer ??= setInterval(() => void this.refresh(), 30_000);
    return () => {
      this.users -= 1;
      if (this.users === 0 && this.timer) {
        clearInterval(this.timer);
        this.timer = null;
      }
    };
  }
}

/** Browser preview numbers. */
const PREVIEW: ScreenTime = {
  totalSecs: 4 * 3600 + 37 * 60,
  apps: [
    { name: "Google Chrome", secs: 2 * 3600 + 5 * 60 },
    { name: "Visual Studio Code", secs: 1 * 3600 + 22 * 60 },
    { name: "Spotify", secs: 41 * 60 },
  ],
};

/** "4h 37m", "52m", or "< 1m". */
export function formatSpan(secs: number): string {
  const m = Math.floor(secs / 60);
  if (m < 1) return "< 1m";
  const h = Math.floor(m / 60);
  return h ? `${h}h ${m % 60}m` : `${m}m`;
}

export const screen = new ScreenTimeStore();
