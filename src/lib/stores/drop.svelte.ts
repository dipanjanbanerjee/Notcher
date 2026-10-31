import {
  dropClearReceived,
  dropOpen,
  dropShare,
  dropStart,
  dropState,
  dropStop,
  dropUnshare,
  inTauri,
  onDropState,
  type DropState,
} from "../bridge";

const EMPTY: DropState = { active: false, url: null, qr: null, folder: "", received: [], shared: [], error: null };

/** Notcher Drop: the phone ↔ PC transfer session and its file lists. */
class DropStore {
  state = $state<DropState>({ ...EMPTY });
  starting = $state(false);

  async refresh() {
    const s = await dropState();
    if (s) this.state = s;
  }

  async start() {
    if (!inTauri) return this.previewStart();
    this.starting = true;
    try {
      const s = await dropStart();
      if (s) this.state = s;
    } finally {
      this.starting = false;
    }
  }

  async stop() {
    if (!inTauri) {
      this.state = { ...this.state, active: false, url: null, qr: null };
      return;
    }
    await dropStop();
  }

  async share(paths: string[]) {
    if (!paths.length) return;
    if (!inTauri) {
      const shared = paths.map((p, i) => ({ id: Date.now() + i, name: p.split(/[\\/]/).pop() ?? p, size: 1_200_000 }));
      this.state = { ...this.state, shared: [...this.state.shared, ...shared] };
      return this.previewStart();
    }
    const s = await dropShare(paths);
    if (s) this.state = s;
  }

  async unshare(id: number) {
    this.state.shared = this.state.shared.filter((s) => s.id !== id);
    await dropUnshare(id);
  }

  async clearReceived() {
    this.state.received = this.state.received.filter((r) => r.status === "receiving");
    await dropClearReceived();
  }

  open = (id: number | null, reveal = false) => dropOpen(id, reveal);

  listen() {
    return onDropState((s) => (this.state = s));
  }

  /** Browser preview: a fake session with a placeholder code. */
  private previewStart() {
    this.state = {
      ...this.state,
      active: true,
      url: "http://192.168.1.24:52814/?k=preview",
      qr: previewQr(),
      folder: "C:\\Users\\you\\Downloads\\Notcher Drop",
    };
  }

  /** Browser preview: fills the lists so the tab can be designed. */
  seedPreview() {
    this.previewStart();
    this.state.received = [
      { id: 1, name: "IMG_2041.HEIC", size: 3_400_000, done: 1_900_000, status: "receiving" },
      { id: 2, name: "Boarding pass.pdf", size: 220_000, done: 220_000, status: "done" },
      { id: 3, name: "Voice memo 12.m4a", size: 1_100_000, done: 1_100_000, status: "done" },
    ];
    this.state.shared = [{ id: 9, name: "Slides final v3.pptx", size: 8_400_000 }];
  }
}

/** A QR-looking pattern for the browser preview only (not scannable). */
function previewQr(): string {
  const n = 25;
  let cells = "";
  let seed = 7;
  const rnd = () => ((seed = (seed * 9301 + 49297) % 233280) / 233280);
  const finder = (x: number, y: number) =>
    `<rect x="${x}" y="${y}" width="7" height="7"/><rect x="${x + 1}" y="${y + 1}" width="5" height="5" fill="#fff"/><rect x="${x + 2}" y="${y + 2}" width="3" height="3"/>`;
  for (let y = 0; y < n; y++)
    for (let x = 0; x < n; x++) {
      const inFinder = (x < 8 && y < 8) || (x > n - 9 && y < 8) || (x < 8 && y > n - 9);
      if (!inFinder && rnd() > 0.52) cells += `<rect x="${x}" y="${y}" width="1" height="1"/>`;
    }
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${n} ${n}" shape-rendering="crispEdges"><rect width="${n}" height="${n}" fill="#fff"/><g>${cells}${finder(0, 0)}${finder(n - 7, 0)}${finder(0, n - 7)}</g></svg>`;
  return "data:image/svg+xml;utf8," + encodeURIComponent(svg);
}

export const drop = new DropStore();
