/**
 * Which widgets the Home tab shows, and in what order. The grid has 4 columns and
 * 2 rows; the media player is two columns wide, everything else one.
 */

export type WidgetId = "clock" | "media" | "focus" | "system" | "clipboard" | "note" | "screen" | "weather";

export const ALL_WIDGETS: { id: WidgetId; label: string; span: 1 | 2 }[] = [
  { id: "clock", label: "Clock", span: 1 },
  { id: "media", label: "Music", span: 2 },
  { id: "focus", label: "Focus", span: 1 },
  { id: "system", label: "System", span: 1 },
  { id: "clipboard", label: "Last copy", span: 1 },
  { id: "note", label: "Note", span: 1 },
  { id: "screen", label: "Screen time", span: 1 },
  { id: "weather", label: "Weather", span: 1 },
];

export const GRID_CELLS = 8;
export const DEFAULT_LAYOUT: WidgetId[] = ["clock", "media", "screen", "focus", "system", "clipboard", "note"];

const spanOf = (id: WidgetId) => ALL_WIDGETS.find((w) => w.id === id)?.span ?? 1;

export function cellsUsed(layout: WidgetId[]): number {
  return layout.reduce((n, id) => n + spanOf(id), 0);
}

/** Keeps known, unique ids that fit in the grid; falls back to the default. */
export function sanitizeLayout(raw: unknown): WidgetId[] {
  if (!Array.isArray(raw)) return [...DEFAULT_LAYOUT];
  const out: WidgetId[] = [];
  for (const id of raw) {
    if (ALL_WIDGETS.some((w) => w.id === id) && !out.includes(id) && cellsUsed([...out, id]) <= GRID_CELLS) {
      out.push(id);
    }
  }
  return out;
}

export function canAdd(layout: WidgetId[], id: WidgetId): boolean {
  return !layout.includes(id) && cellsUsed(layout) + spanOf(id) <= GRID_CELLS;
}

export function addWidget(layout: WidgetId[], id: WidgetId): WidgetId[] {
  return canAdd(layout, id) ? [...layout, id] : layout;
}

export function removeWidget(layout: WidgetId[], id: WidgetId): WidgetId[] {
  return layout.filter((w) => w !== id);
}

/** Moves a widget one place earlier (-1) or later (+1). */
export function moveWidget(layout: WidgetId[], id: WidgetId, by: -1 | 1): WidgetId[] {
  const i = layout.indexOf(id);
  const j = i + by;
  if (i < 0 || j < 0 || j >= layout.length) return layout;
  const next = [...layout];
  [next[i], next[j]] = [next[j], next[i]];
  return next;
}

/** Moves a widget to `index` (used while dragging), shifting the others along. */
export function placeWidget(layout: WidgetId[], id: WidgetId, index: number): WidgetId[] {
  const from = layout.indexOf(id);
  if (from < 0) return layout;
  const to = Math.max(0, Math.min(layout.length - 1, index));
  if (to === from) return layout;
  const next = layout.filter((w) => w !== id);
  next.splice(to, 0, id);
  return next;
}

const KEY = "notcher.home.widgets";

/** Set once a saved layout has been offered the widgets added since it was saved. */
const UPGRADED_KEY = "notcher.home.v2";

/**
 * Layouts saved before the grid grew to four columns get the new Screen time widget,
 * once. Removing it afterwards sticks.
 */
export function upgradeLayout(layout: WidgetId[]): WidgetId[] {
  if (layout.includes("screen")) return layout;
  const at = layout.indexOf("media");
  const next = [...layout];
  next.splice(at < 0 ? next.length : at + 1, 0, "screen");
  return cellsUsed(next) <= GRID_CELLS ? next : layout;
}

export function loadLayout(): WidgetId[] {
  try {
    const raw = localStorage.getItem(KEY);
    if (!raw) return [...DEFAULT_LAYOUT];
    let layout = sanitizeLayout(JSON.parse(raw));
    if (!localStorage.getItem(UPGRADED_KEY)) {
      layout = upgradeLayout(layout);
      localStorage.setItem(UPGRADED_KEY, "1");
      saveLayout(layout);
    }
    return layout;
  } catch {
    return [...DEFAULT_LAYOUT];
  }
}

export function saveLayout(layout: WidgetId[]) {
  try {
    localStorage.setItem(KEY, JSON.stringify(layout));
  } catch {
    /* not persisted */
  }
}
