/**
 * Finds newer versions of Notcher on GitHub Releases. Installing is done by the Rust
 * side (`install_update`), which only accepts installers from this project's releases.
 */

export interface Release {
  version: string;
  /** The installer to download. */
  url: string;
  /** The release page, for "what's new". */
  page: string;
}

const LATEST = "https://api.github.com/repos/dipanjanbanerjee/notcher/releases/latest";

/** Compares "1.2.10" with "1.2.9": negative, zero or positive. A leading "v" is ignored. */
export function compareVersions(a: string, b: string): number {
  const parts = (v: string) => v.replace(/^v/i, "").split(/[.-]/).map((p) => Number.parseInt(p, 10) || 0);
  const x = parts(a);
  const y = parts(b);
  for (let i = 0; i < Math.max(x.length, y.length); i++) {
    const d = (x[i] ?? 0) - (y[i] ?? 0);
    if (d) return d;
  }
  return 0;
}

/** Reads a GitHub "latest release" response. */
export function parseRelease(json: unknown): Release | null {
  const j = json as { tag_name?: unknown; html_url?: unknown; draft?: unknown; prerelease?: unknown; assets?: { name?: unknown; browser_download_url?: unknown }[] } | null;
  if (!j || typeof j.tag_name !== "string" || j.draft || j.prerelease || !Array.isArray(j.assets)) return null;
  const exe =
    j.assets.find((a) => a.name === "Notcher-Setup.exe") ??
    j.assets.find((a) => typeof a.name === "string" && /setup\.exe$/i.test(a.name));
  if (!exe || typeof exe.browser_download_url !== "string") return null;
  const version = j.tag_name.replace(/^v/i, "");
  if (!/^\d+(\.\d+){0,3}$/.test(version)) return null;
  return { version, url: exe.browser_download_url, page: typeof j.html_url === "string" ? j.html_url : "" };
}

/** The newest release when it is newer than `current`, otherwise null. Throws when offline. */
export async function findUpdate(current: string): Promise<Release | null> {
  const res = await fetch(LATEST, { headers: { Accept: "application/vnd.github+json" }, signal: AbortSignal.timeout(15_000) });
  if (!res.ok) throw new Error(`HTTP ${res.status}`);
  const release = parseRelease(await res.json());
  return release && compareVersions(release.version, current) > 0 ? release : null;
}
