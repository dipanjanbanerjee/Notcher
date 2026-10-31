<script lang="ts">
  import { onMount } from "svelte";
  import { inTauri, onNetSpeed, type NetSpeed } from "../lib/bridge";
  import { formatBytes, formatRate } from "../lib/stores/vitals.svelte";

  let speed = $state<NetSpeed>({ rx: 0, tx: 0, lightTaskbar: false, today: null, size: "medium", left: false });

  onMount(() => {
    if (!inTauri) {
      // Browser preview: /netspeed.html?size=large&left&light
      const q = new URLSearchParams(location.search);
      const size = q.get("size");
      speed = {
        rx: 2.4 * 2 ** 20,
        tx: 120 * 2 ** 10,
        lightTaskbar: q.has("light"),
        today: q.has("notoday") ? null : 1.86 * 2 ** 30,
        size: size === "small" || size === "large" ? size : "medium",
        left: q.has("left"),
      };
    }
    const off = onNetSpeed((s) => (speed = s));
    return () => void off.then((f) => f());
  });
</script>

<!-- Upload on top and download below, then today's usage. -->
<div class="meter {speed.size}" class:light={speed.lightTaskbar} class:left={speed.left}>
  <div class="col">
    <span><i>↑</i>{formatRate(speed.tx)}</span>
    <span><i>↓</i>{formatRate(speed.rx)}</span>
  </div>
  {#if speed.today !== null}
    <div class="col today">
      <span class="label">Today</span>
      <span>{formatBytes(speed.today, 2)}</span>
    </div>
  {/if}
</div>

<style>
  :global(html),
  :global(body) {
    margin: 0;
    height: 100%;
    background: transparent;
    overflow: hidden;
    user-select: none;
  }
  .meter {
    --fs: 12.5px;
    height: 100vh;
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 10px;
    padding: 0 2px;
    font: var(--fs) / 1.22 "Segoe UI Variable Text", "Segoe UI", system-ui, sans-serif;
    font-variant-numeric: tabular-nums;
    color: #fff;
    white-space: nowrap;
  }
  .small { --fs: 11px; }
  .large { --fs: 14px; font-weight: 500; }
  .meter.left { justify-content: flex-start; }
  .meter.light { color: #1b1b1b; }
  .col {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
  }
  .left .col { align-items: flex-start; }
  /* "Today" and its total sit centred over each other, on either side. */
  .col.today, .left .col.today { align-items: center; text-align: center; }
  .today { padding-left: 10px; border-left: 1px solid rgb(127 127 127 / 0.45); }
  /* Mirrored at the left end of the taskbar: today's usage first. */
  .left .today { order: -1; padding: 0 10px 0 0; border-left: 0; border-right: 1px solid rgb(127 127 127 / 0.45); }
  .label,
  i {
    font-style: normal;
    opacity: 0.7;
  }
  i { margin-right: 3px; }
</style>
