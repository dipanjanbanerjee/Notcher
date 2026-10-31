/**
 * Geometry of the island shape.
 *
 * The island hangs from the top edge of the screen. Its top corners flare outward
 * into concave "shoulders" so it reads as part of the bezel, and its bottom corners
 * use a smoothed (squircle-like) curve rather than a plain circular arc.
 */

export interface IslandGeometry {
  /** Width of the island body, excluding the shoulders. */
  width: number;
  height: number;
  /** Bottom corner radius. */
  radius: number;
  /** Size of the concave flare at each top corner. */
  shoulder: number;
}

export interface Rect {
  x: number;
  y: number;
  width: number;
  height: number;
}

/**
 * How far the smoothed corner curve extends compared with a circular corner of the
 * same radius. Values above 1 give the softer, continuous-curvature look.
 */
const CORNER_SMOOTHING = 1.28;

const round = (n: number) => Math.round(n * 100) / 100;

/** Keeps geometry drawable: non-negative, and corners never larger than the body allows. */
export function normalize(g: IslandGeometry): IslandGeometry {
  const width = Math.max(0, g.width);
  const height = Math.max(0, g.height);
  const shoulder = Math.max(0, Math.min(g.shoulder, height / 2, width / 4));
  const maxCorner = Math.max(0, Math.min(width / 2, height - shoulder));
  const radius = Math.max(0, Math.min(g.radius, maxCorner / CORNER_SMOOTHING));
  return { width, height, radius, shoulder };
}

/**
 * SVG path for the island, horizontally centred on `centerX`, hanging from y = 0.
 */
export function islandPath(geometry: IslandGeometry, centerX: number): string {
  const { width, height, radius, shoulder } = normalize(geometry);
  const left = centerX - width / 2;
  const right = centerX + width / 2;
  const k = radius * CORNER_SMOOTHING;
  // Control-point inset for the smoothed corner.
  const c = k * 0.24;

  const d = [
    `M ${round(left - shoulder)} 0`,
    `L ${round(right + shoulder)} 0`,
    // Top-right shoulder: concave curve from the bezel down into the side.
    `Q ${round(right)} 0 ${round(right)} ${round(shoulder)}`,
    `L ${round(right)} ${round(height - k)}`,
    // Bottom-right smoothed corner.
    `C ${round(right)} ${round(height - c)} ${round(right - c)} ${round(height)} ${round(right - k)} ${round(height)}`,
    `L ${round(left + k)} ${round(height)}`,
    // Bottom-left smoothed corner.
    `C ${round(left + c)} ${round(height)} ${round(left)} ${round(height - c)} ${round(left)} ${round(height - k)}`,
    `L ${round(left)} ${round(shoulder)}`,
    // Top-left shoulder.
    `Q ${round(left)} 0 ${round(left - shoulder)} 0`,
    "Z",
  ];
  return d.join(" ");
}

/** Bounding box of the island including its shoulders. */
export function islandBounds(geometry: IslandGeometry, centerX: number): Rect {
  const { width, height, shoulder } = normalize(geometry);
  return {
    x: centerX - width / 2 - shoulder,
    y: 0,
    width: width + shoulder * 2,
    height,
  };
}

/** Smallest rectangle containing both rectangles. */
export function union(a: Rect, b: Rect): Rect {
  const x = Math.min(a.x, b.x);
  const y = Math.min(a.y, b.y);
  return {
    x,
    y,
    width: Math.max(a.x + a.width, b.x + b.width) - x,
    height: Math.max(a.y + a.height, b.y + b.height) - y,
  };
}

/** Grows a rectangle sideways and downward (the island never extends above y = 0). */
export function inflate(r: Rect, sides: number, bottom: number): Rect {
  return { x: r.x - sides, y: r.y, width: r.width + sides * 2, height: r.height + bottom };
}

/** Rounds outward to whole pixels so the input region never clips the visible shape. */
export function snapOut(r: Rect): Rect {
  const x = Math.floor(r.x);
  const y = Math.floor(r.y);
  return {
    x,
    y,
    width: Math.ceil(r.x + r.width) - x,
    height: Math.ceil(r.y + r.height) - y,
  };
}

export function sameRect(a: Rect | null, b: Rect): boolean {
  return (
    !!a && a.x === b.x && a.y === b.y && a.width === b.width && a.height === b.height
  );
}
