<script lang="ts">
  import { onMount } from "svelte";
  import { inTauri, onNetSpeed, type NetSpeed } from "../lib/bridge";
  import { formatRate } from "../lib/stores/vitals.svelte";

  let speed = $state<NetSpeed>({ rx: 0, tx: 0, lightTaskbar: false });

  onMount(() => {
    if (!inTauri) speed = { rx: 2.4 * 2 ** 20, tx: 120 * 2 ** 10, lightTaskbar: false };
    const off = onNetSpeed((s) => (speed = s));
    return () => void off.then((f) => f());
  });
</script>

<!-- Upload on top, download below, like most taskbar meters. -->
<div class="meter" class:light={speed.lightTaskbar}>
  <span><i>↑</i>{formatRate(speed.tx)}</span>
  <span><i>↓</i>{formatRate(speed.rx)}</span>
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
    height: 100vh;
    display: flex;
    flex-direction: column;
    justify-content: center;
    align-items: flex-end;
    padding-right: 2px;
    font: 11px/1.25 "Segoe UI Variable Text", "Segoe UI", system-ui, sans-serif;
    font-variant-numeric: tabular-nums;
    color: #fff;
    white-space: nowrap;
  }
  .meter.light {
    color: #1b1b1b;
  }
  i {
    font-style: normal;
    opacity: 0.7;
    margin-right: 3px;
  }
</style>
