/** "3:07" or "1:02:45". Negative and non-finite input count as zero. */
export function formatDuration(ms: number): string {
  const total = Number.isFinite(ms) ? Math.max(0, Math.floor(ms / 1000)) : 0;
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  const ss = String(s).padStart(2, "0");
  return h > 0 ? `${h}:${String(m).padStart(2, "0")}:${ss}` : `${m}:${ss}`;
}

export function formatClock(date: Date, use24h: boolean, seconds = false): string {
  const h = date.getHours();
  const m = String(date.getMinutes()).padStart(2, "0");
  const s = seconds ? `:${String(date.getSeconds()).padStart(2, "0")}` : "";
  if (use24h) return `${String(h).padStart(2, "0")}:${m}${s}`;
  return `${h % 12 || 12}:${m}${s}`;
}

export function meridiem(date: Date): string {
  return date.getHours() < 12 ? "AM" : "PM";
}

/** "Sat Oct 31": short weekday, short month, day, always in that order. */
export function formatShortDate(date: Date, locale?: string): string {
  const weekday = date.toLocaleDateString(locale, { weekday: "short" });
  const month = date.toLocaleDateString(locale, { month: "short" });
  return `${weekday} ${month} ${date.getDate()}`;
}

export function formatDate(date: Date, locale?: string): string {
  return date.toLocaleDateString(locale, { weekday: "long", day: "numeric", month: "long" });
}

/**
 * Estimates the current playback position from the last reported one, so the
 * progress bar moves smoothly between updates from the player.
 */
export function estimatePosition(
  reportedMs: number,
  reportedAt: number,
  now: number,
  playing: boolean,
  durationMs: number,
): number {
  const elapsed = playing ? Math.max(0, now - reportedAt) : 0;
  const pos = reportedMs + elapsed;
  return durationMs > 0 ? Math.min(pos, durationMs) : pos;
}
