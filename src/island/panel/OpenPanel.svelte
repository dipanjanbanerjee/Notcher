<script lang="ts" module>
  export type TabId = "home" | "clips" | "jot" | "focus" | "convert" | "weather" | "vitals" | "drop";
  export const TAB_IDS: readonly TabId[] = ["home", "clips", "jot", "focus", "convert", "weather", "vitals", "drop"];
</script>

<script lang="ts">
  import Icon, { type IconName } from "../../components/Icon.svelte";
  import { openSettings, type NowPlaying } from "../../lib/bridge";
  import ClipsTab from "./ClipsTab.svelte";
  import DropTab from "./DropTab.svelte";
  import FocusTab from "./FocusTab.svelte";
  import HomeTab from "./HomeTab.svelte";
  import JotTab from "./JotTab.svelte";
  import ConvertTab from "./ConvertTab.svelte";
  import WeatherTab from "./WeatherTab.svelte";
  import VitalsTab from "./VitalsTab.svelte";

  let { track, clock, clock24h, tab = $bindable(), dragging = false }: {
    track: NowPlaying | null;
    clock: Date;
    clock24h: boolean;
    tab: TabId;
    /** Files are being dragged over the island. */
    dragging?: boolean;
  } = $props();

  const TABS: { id: TabId; label: string; icon: IconName }[] = [
    { id: "home", label: "Home", icon: "home" },
    { id: "clips", label: "Clipboard", icon: "clipboard" },
    { id: "jot", label: "Notes", icon: "pen" },
    { id: "focus", label: "Focus", icon: "timer" },
    { id: "convert", label: "Convert", icon: "tools" },
    { id: "weather", label: "Weather", icon: "partly" },
    { id: "vitals", label: "System", icon: "pulse" },
    { id: "drop", label: "Drop", icon: "beam" },
  ];

  let editingHome = $state(false);
  $effect(() => {
    if (tab !== "home") editingHome = false;
  });

  /** Direction of the last tab change, for the slide animation. */
  let direction = $state(1);
  function go(next: TabId) {
    if (next === tab) return;
    direction = TABS.findIndex((t) => t.id === next) > TABS.findIndex((t) => t.id === tab) ? 1 : -1;
    tab = next;
  }

  function onKey(e: KeyboardEvent) {
    // Ctrl+1…8 switches tabs while the island is open.
    if ((e.ctrlKey || e.metaKey) && /^[1-8]$/.test(e.key)) {
      e.preventDefault();
      go(TABS[Number(e.key) - 1].id);
    }
  }
</script>

<svelte:window onkeydown={onKey} />

<div class="panel">
  <nav aria-label="Sections">
    <div class="tabs" role="tablist">
      {#each TABS as t (t.id)}
        <button role="tab" aria-selected={tab === t.id} aria-label={t.label} title={t.label} onclick={(e) => { e.stopPropagation(); go(t.id); }}>
          <Icon name={t.icon} size={15} />
          <span class="label"><span>{t.label}</span></span>
        </button>
      {/each}
    </div>
    <div class="tools">
    {#if tab === "home"}
      <button
        class="tool"
        class:on={editingHome}
        aria-label={editingHome ? "Done editing Home" : "Edit Home widgets"}
        title={editingHome ? "Done" : "Edit widgets"}
        onclick={(e) => { e.stopPropagation(); editingHome = !editingHome; }}
      >
        <Icon name={editingHome ? "check" : "pen"} size={16} />
      </button>
    {/if}
    <button class="gear" aria-label="Settings" onclick={(e) => { e.stopPropagation(); void openSettings(); }}>
      <Icon name="gear" size={17} />
    </button>
    </div>
  </nav>
  <div class="body">
    {#key tab}
      <div class="page" style:--dir={direction}>
        {#if tab === "home"}
          <HomeTab {track} {clock} {clock24h} ongo={go} bind:editing={editingHome} />
        {:else if tab === "clips"}
          <ClipsTab />
        {:else if tab === "jot"}
          <JotTab />
        {:else if tab === "focus"}
          <FocusTab />
        {:else if tab === "convert"}
          <ConvertTab />
        {:else if tab === "weather"}
          <WeatherTab />
        {:else if tab === "drop"}
          <DropTab {dragging} />
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
    display: inline-flex; align-items: center;
    height: 30px; padding: 0 8px; border: 0; border-radius: 9px;
    background: transparent; color: var(--muted); font: inherit; font-size: 12.5px; cursor: pointer;
    transition: background 280ms cubic-bezier(0.2, 0.9, 0.25, 1), color 200ms, padding 320ms cubic-bezier(0.2, 0.9, 0.25, 1);
  }
  .tabs button:hover { color: var(--fg); background: color-mix(in srgb, var(--surface-2) 50%, transparent); }
  .tabs button[aria-selected="true"] { background: var(--surface-2); color: var(--fg); padding: 0 11px 0 9px; }
  .tabs button[aria-selected="true"] :global(svg) { color: var(--glow, var(--accent)); }
  /* Only the open tab shows its name; the label slides open instead of popping in. */
  .label {
    display: grid; grid-template-columns: 0fr; opacity: 0;
    transition: grid-template-columns 320ms cubic-bezier(0.2, 0.9, 0.25, 1), opacity 200ms;
  }
  .label > span { overflow: hidden; white-space: nowrap; padding-left: 0; transition: padding 320ms cubic-bezier(0.2, 0.9, 0.25, 1); }
  [aria-selected="true"] .label { grid-template-columns: 1fr; opacity: 1; transition-delay: 0ms, 80ms; }
  [aria-selected="true"] .label > span { padding-left: 6px; }
  @media (prefers-reduced-motion: reduce) {
    .tabs button, .label, .label > span { transition: none; }
  }
  .tools { margin-left: auto; display: flex; gap: 2px; }
  .gear, .tool {
    width: 32px; height: 32px; display: grid; place-items: center;
    border: 0; border-radius: 50%; background: transparent; color: var(--muted); cursor: pointer;
    transition: background 160ms, color 160ms, transform 400ms cubic-bezier(0.2, 0.8, 0.2, 1);
  }
  .gear:hover, .tool:hover { background: var(--surface-2); color: var(--fg); }
  .gear:hover { transform: rotate(60deg); }
  .tool.on { background: var(--surface-2); color: var(--glow, var(--accent)); }
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
