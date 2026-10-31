import { onVitals, setVitalsActive, vitalsSnapshot, type Vitals } from "../bridge";

class VitalsStore {
  current = $state<Vitals | null>(null);
  private active = false;

  /** Live updates flow only while the panel is open. */
  async setActive(active: boolean) {
    if (active === this.active) return;
    this.active = active;
    await setVitalsActive(active);
    if (active) {
      const snap = await vitalsSnapshot();
      if (snap) this.current = snap;
    }
  }

  listen() {
    return onVitals((v) => (this.current = v));
  }
}

export const vitals = new VitalsStore();

export function formatBytes(bytes: number, digits = 1): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  const i = Math.min(units.length - 1, Math.floor(Math.log(bytes) / Math.log(1024)));
  const v = bytes / 1024 ** i;
  return `${v >= 100 || i === 0 ? Math.round(v) : v.toFixed(digits)} ${units[i]}`;
}

export const formatRate = (bytesPerSec: number) => `${formatBytes(bytesPerSec)}/s`;
