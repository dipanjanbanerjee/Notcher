<script lang="ts">
  import Icon from "../../components/Icon.svelte";
  import Sparkline from "./Sparkline.svelte";
  import { formatBytes, vitals } from "../../lib/stores/vitals.svelte";

  const v = $derived(vitals.current);
  const memPct = $derived(v && v.memTotal ? (v.memUsed / v.memTotal) * 100 : 0);
  const diskPct = $derived(v && v.diskTotal ? (v.diskUsed / v.diskTotal) * 100 : 0);
</script>

{#if v}
  <div class="grid">
    <section class="card">
      <header><span>CPU</span><strong>{Math.round(v.cpu)}%</strong></header>
      <Sparkline values={v.cpuHistory} width={250} height={44} />
      <span class="muted">Last minute</span>
    </section>
    <section class="card">
      <header><span>Memory</span><strong>{Math.round(memPct)}%</strong></header>
      <Sparkline values={v.memHistory} width={250} height={44} />
      <span class="muted">{formatBytes(v.memUsed)} of {formatBytes(v.memTotal)}</span>
    </section>
    <section class="card">
      <header><span>Disk</span><strong>{Math.round(diskPct)}%</strong></header>
      <div class="bar"><div style:width="{diskPct}%"></div></div>
      <span class="muted">{formatBytes(v.diskTotal - v.diskUsed)} free of {formatBytes(v.diskTotal)}</span>
    </section>
    <section class="card">
      <header>
        <span>Battery</span>
        {#if v.battery}<strong>{Math.round(v.battery.percent)}%</strong>{/if}
      </header>
      {#if v.battery}
        <div class="bar"><div style:width="{v.battery.percent}%"></div></div>
        <span class="battery">
          {#if v.battery.charging}<Icon name="bolt" size={13} />{/if}
          {v.battery.charging ? "Charging" : "On battery"}
        </span>
      {:else}
        <span class="battery"><Icon name="bolt" size={13} />Plugged in, no battery</span>
      {/if}
    </section>
  </div>
{:else}
  <p class="empty">Reading system stats…</p>
{/if}

<style>
  .grid { height: 100%; display: grid; grid-template-columns: 1fr 1fr; grid-template-rows: 1fr 1fr; gap: 10px; }
  .card {
    display: flex; flex-direction: column; justify-content: space-between; gap: 4px;
    padding: 10px 12px; border-radius: 14px;
    background: var(--surface-1); box-shadow: inset 0 0 0 1px var(--hairline);
    min-width: 0; overflow: hidden;
  }
  header { display: flex; justify-content: space-between; align-items: baseline; font-size: 12px; color: var(--muted); }
  strong { color: var(--fg); font-size: 16px; font-variant-numeric: tabular-nums; }
  .muted { color: var(--muted); font-size: 11.5px; }
  .bar { height: 6px; border-radius: 3px; background: var(--surface-2); overflow: hidden; }
  .bar div { height: 100%; background: var(--glow, var(--accent)); border-radius: 3px; transition: width 400ms ease; }
  .battery { display: inline-flex; align-items: center; gap: 4px; font-size: 12px; color: var(--muted); }
  .battery :global(svg) { color: var(--glow, var(--accent)); }
  .empty { margin: auto; color: var(--muted); }
</style>
