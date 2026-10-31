import { describe, expect, it } from "vitest";
import { focus } from "./focus.svelte";

describe("water reminder", () => {
  it("counts only time at the PC, in capped steps", () => {
    let t = Date.now() + 1000;
    focus.setWater(0);
    focus.setWater(1);
    let fired = 0;
    // 50 s at the PC, then 10 minutes away, then 10 s back.
    for (let i = 0; i < 50; i++) if (focus.update((t += 1000), true).water) fired++;
    for (let i = 0; i < 600; i++) if (focus.update((t += 1000), false).water) fired++;
    expect(fired).toBe(0);
    for (let i = 0; i < 10; i++) if (focus.update((t += 1000), true).water) fired++;
    expect(fired).toBe(1);
    // A long jump (sleep) counts as at most five seconds.
    expect(focus.update((t += 3_600_000), true).water).toBe(false);
  });

  it("is off at zero", () => {
    let t = Date.now() + 10_000_000;
    focus.setWater(0);
    for (let i = 0; i < 200; i++) expect(focus.update((t += 1000), true).water).toBe(false);
  });
});
