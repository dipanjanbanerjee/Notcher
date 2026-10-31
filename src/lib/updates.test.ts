import { describe, expect, it } from "vitest";
import { compareVersions, parseRelease } from "./updates";

describe("updates", () => {
  it("compares versions numerically", () => {
    expect(compareVersions("1.0.1", "1.0.0")).toBeGreaterThan(0);
    expect(compareVersions("v1.2.10", "1.2.9")).toBeGreaterThan(0);
    expect(compareVersions("1.0", "1.0.0")).toBe(0);
    expect(compareVersions("0.9.9", "1.0.0")).toBeLessThan(0);
  });

  it("finds the installer in a release", () => {
    const r = parseRelease({
      tag_name: "v1.0.1",
      html_url: "https://github.com/x/y/releases/tag/v1.0.1",
      assets: [
        { name: "Notcher_1.0.1_x64-setup.exe", browser_download_url: "https://example/a.exe" },
        { name: "Notcher-Setup.exe", browser_download_url: "https://example/b.exe" },
      ],
    });
    expect(r).toEqual({ version: "1.0.1", url: "https://example/b.exe", page: "https://github.com/x/y/releases/tag/v1.0.1" });
    expect(parseRelease({ tag_name: "v2", draft: true, assets: [] })).toBeNull();
    expect(parseRelease({ tag_name: "v2", assets: [{ name: "x.msi", browser_download_url: "u" }] })).toBeNull();
    expect(parseRelease({ tag_name: "nightly", assets: [{ name: "Notcher-Setup.exe", browser_download_url: "u" }] })).toBeNull();
  });
});
