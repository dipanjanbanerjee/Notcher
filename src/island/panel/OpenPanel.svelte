<script lang="ts" module>
  export type TabId = "home" | "clips" | "jot" | "focus" | "vitals";
</script>

<script lang="ts">
  import Icon, { type IconName } from "../../components/Icon.svelte";
  import { openSettings, type NowPlaying } from "../../lib/bridge";
  import ClipsTab from "./ClipsTab.svelte";
  import FocusTab from "./FocusTab.svelte";
  import HomeTab from "./HomeTab.svelte";
  import JotTab from "./JotTab.svelte";
  import VitalsTab from "./VitalsTab.svelte";

  let { track, clock, clock24h, tab = $bindable() }: {
    track: NowPlaying | null;
    clock: Date;
    clock24h: boolean;
    tab: TabId;
  } = $props();

  const TABS: { id: TabId; label: string; icon: IconName }[] = [
    { id: "home", label: "Home", icon: "home" },
    { id: "clips", label: "Clipboard", icon: "clipboard" },
    { id: "jot", label: "Notes", icon: "pen" },
    { id: "focus", label: "Focus", icon: "timer" },
    { id: "vitals", label: "System", icon: "pulse" },
  ];

  /** Direction of the last tab change, for the slide animation. */
  let direction = $state(1);
  function go(next: TabId) {
    if (next === tab) return;
    direction = TABS.findIndex((t) => t.id === next) > TABS.findIndex((t) => t.id === tab) ? 1 : -1;
    tab = next;
  }

  function onKey(e: KeyboardEvent) {
    // Ctrl+1…5 switches tabs while the island is open.
    if ((e.ctrlKey || e.metaKey) && /^[1-5]$/.test(e.key)) {
      e.preventDefault();
      go(TABS[Number(e.key) - 1].id);
    }
  }
</script>

<svelte:window onkeydown={onKey} />

<div class="panel">
  <nav aria-label="Tools">
    <div class="tabs" role="tablist">
      {#each TABS as t (t.id)}
        <button role="tab" aria-selected={tab === t.id} title={t.label} onclick={(e) => { e.stopPropagation(); go(t.id); }}>
          <Icon name={t.icon} size={15} />
          <span>{t.label}</span>
        </button>
      {/each}
    </div>
    <button class="gear" aria-label="Settings" onclick={(e) => { e.stopPropagation(); void openSettings(); }}>
      <Icon name="gear" size={17} />
    </button>
  </nav>
  <div class="body">
    {#key tab}
      <div class="page" style:--dir={direction}>
        {#if tab === "home"}
          <HomeTab {track} {clock} {clock24h} ongo={go} />
        {:else if tab === "clips"}
          <ClipsTab />
        {:else if tab === "jot"}
          <JotTab />
        {:else if tab === "focus"}
          <FocusTab />
        {:else}
          <VitalsTab />
        {/if}
      </div>
    {/key}
  </div>
</div>

<style>
  .panel { height: 100%; display: flex; flex-direction: column; gap: 12px; padding: 14px 18px 18px; }
  nav { display: flex; align-items: center; gap: 8px; }
  .tabs { display: flex; gap: 2px; padding: 3px; border-radius: 12px; background: var(--surface-1); }
  .tabs button {
    display: inline-flex; align-items: center; gap: 6px;
    height: 30px; padding: 0 10px; border: 0; border-radius: 9px;
    background: transparent; color: var(--muted); font: inherit; font-size: 12.5px; cursor: pointer;
    transition: background 180ms, color 180ms;
  }
  .tabs button:hover { color: var(--fg); }
  .tabs button[aria-selected="true"] { background: var(--surface-2); color: var(--fg); }
  .tabs button[aria-selected="true"] :global(svg) { color: var(--glow, var(--accent)); }
  .gear {
    margin-left: auto; width: 32px; height: 32px; display: grid; place-items: center;
    border: 0; border-radius: 50%; background: transparent; color: var(--muted); cursor: pointer;
    transition: background 160ms, color 160ms, transform 400ms cubic-bezier(0.2, 0.8, 0.2, 1);
  }
  .gear:hover { background: var(--surface-2); color: var(--fg); transform: rotate(60deg); }
  .body { position: relative; flex: 1; min-height: 0; }
  .page {
    position: absolute;
    inset: 0;
    animation: page-in 260ms cubic-bezier(0.2, 0.9, 0.25, 1);
  }
  @keyframes page-in {
    from { opacity: 0; transform: translateX(calc(var(--dir) * 18px)); }
  }
  @media (prefers-reduced-motion: reduce) {
    .page { animation: none; }
  }
</style>
