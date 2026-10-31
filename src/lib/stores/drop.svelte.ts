import {
  dropClearReceived,
  dropOpen,
  dropShare,
  dropStart,
  dropState,
  dropStop,
  dropThumb,
  dropUnshare,
  inTauri,
  onDropState,
  type DropState,
} from "../bridge";

const EMPTY: DropState = {
  active: false,
  url: null,
  qr: null,
  folder: "",
  received: [],
  shared: [],
  error: null,
  device: null,
  deviceSeenAt: 0,
};

const IMAGE = /\.(png|jpe?g|webp|gif|bmp)$/i;

/** Notcher Drop: the phone ↔ PC transfer session and its file lists. */
class DropStore {
  state = $state<DropState>({ ...EMPTY });
  starting = $state(false);
  /** Image previews by item id (null when there is none). */
  thumbs = $state<Record<number, string | null>>({});
  /** The user pressed Stop, so don't start again until they leave the Drop tab. */
  private held = false;

  /** Starts sharing when the Drop tab is showing, so the QR code is simply there. */
  ensure() {
    if (this.held || this.state.active || this.starting || this.state.error) return;
    void this.start();
  }

  /** The Drop tab is no longer showing: the next visit starts sharing again. */
  release() {
    this.held = false;
    if (this.state.error) this.state.error = null;
  }

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
    this.held = true;
    if (!inTauri) {
      this.state = { ...this.state, active: false, url: null, qr: null };
      return;
    }
    await dropStop();
  }

  async share(paths: string[]) {
    if (!paths.length) return;
    if (!inTauri) {
      const shared = paths.map((p, i) => ({ id: Date.now() + i, name: p.split(/[\\/]/).pop() ?? p, size: 1_200_000, downloads: 0, at: Date.now() }));
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

  /** Starts loading an image preview for each item that needs one (once per item). */
  loadThumbs(list: { id: number; name: string }[]) {
    for (const { id, name } of list) {
      if (!IMAGE.test(name) || id in this.thumbs) continue;
      this.thumbs[id] = null;
      void dropThumb(id).then((t) => (this.thumbs[id] = t ?? null));
    }
  }

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
  seedPreview(connected = true) {
    this.previewStart();
    const t = Date.now();
    this.state.device = connected ? "Pixel 7" : null;
    this.state.deviceSeenAt = connected ? t : 0;
    this.state.received = connected
      ? [
          { id: 1, name: "IMG_2041.jpg", size: 3_400_000, done: 1_900_000, status: "receiving", rate: 4_200_000, at: t - 2_000 },
          { id: 2, name: "Boarding pass.pdf", size: 220_000, done: 220_000, status: "done", rate: 0, at: t - 60_000 },
          { id: 3, name: "Voice memo 12.m4a", size: 1_100_000, done: 1_100_000, status: "done", rate: 0, at: t - 600_000 },
        ]
      : [];
    this.state.shared = connected ? [{ id: 9, name: "Slides final v3.pptx", size: 8_400_000, downloads: 1, at: t - 30_000 }] : [];
    const art =
      "data:image/svg+xml;utf8," +
      encodeURIComponent(
        `<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64"><defs><linearGradient id="g" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#7dd3fc"/><stop offset="1" stop-color="#f59e0b"/></linearGradient></defs><rect width="64" height="64" fill="url(#g)"/><circle cx="46" cy="18" r="7" fill="#fff7"/><path d="M0 52l18-16 14 10 12-8 20 14v12H0z" fill="#1e3a5f"/></svg>`,
      );
    this.thumbs = { 1: art };
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
