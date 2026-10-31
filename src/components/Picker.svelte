<script lang="ts" module>
  export interface PickerOption {
    id: string;
    label: string;
    /** Shown dimmed after the label, e.g. a unit symbol. */
    hint?: string;
  }
</script>

<script lang="ts">
  /**
   * A dropdown drawn inside the island. Native <select> menus open as a separate
   * system window: in the island they showed blank items until hovered, and moving
   * into them counted as leaving the island.
   */
  import { tick } from "svelte";

  let {
    options,
    value,
    onchange,
    label,
    searchable = false,
    up = false,
    width,
    sheet,
  }: {
    options: PickerOption[];
    value: string;
    onchange: (id: string) => void;
    label: string;
    searchable?: boolean;
    /** Open upwards (for pickers near the bottom). */
    up?: boolean;
    /** Width of the list, when it should differ from the button. */
    width?: number;
    /** Open as a tall sheet over the left or right half of the nearest positioned
     *  parent, for long lists that would not fit below the button. */
    sheet?: "left" | "right";
  } = $props();

  let open = $state(false);
  let query = $state("");
  let active = $state(0);
  let root: HTMLDivElement;
  let list = $state<HTMLUListElement>();
  let search = $state<HTMLInputElement>();

  const current = $derived(options.find((o) => o.id === value));
  const shown = $derived.by(() => {
    const q = query.trim().toLowerCase();
    return q ? options.filter((o) => `${o.id} ${o.label} ${o.hint ?? ""}`.toLowerCase().includes(q)) : options;
  });

  async function show() {
    query = "";
    open = true;
    active = Math.max(0, options.findIndex((o) => o.id === value));
    await tick();
    if (searchable) search?.focus();
    reveal();
  }

  function close(focusButton = false) {
    open = false;
    if (focusButton) root?.querySelector<HTMLButtonElement>(".current")?.focus();
  }

  function pick(id: string) {
    if (id !== value) onchange(id);
    close(true);
  }

  function reveal() {
    list?.querySelector<HTMLElement>(`[data-i="${active}"]`)?.scrollIntoView({ block: "nearest" });
  }

  function onKey(e: KeyboardEvent) {
    if (!open) {
      if (e.key === "ArrowDown" || e.key === "ArrowUp" || e.key === "Enter" || e.key === " ") {
        e.preventDefault();
        void show();
      }
      return;
    }
    if (e.key === "Escape") {
      // Close the list only, not the island.
      e.stopPropagation();
      close(true);
    } else if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      const n = shown.length;
      if (n) active = (active + (e.key === "ArrowDown" ? 1 : n - 1)) % n;
      reveal();
    } else if (e.key === "Enter") {
      e.preventDefault();
      const o = shown[active];
      if (o) pick(o.id);
    } else if (e.key === "Tab") {
      close();
    }
  }

  $effect(() => {
    if (!open) return;
    const outside = (e: PointerEvent) => {
      if (!root.contains(e.target as Node)) close();
    };
    document.addEventListener("pointerdown", outside, true);
    return () => document.removeEventListener("pointerdown", outside, true);
  });
  $effect(() => {
    void query;
    active = 0;
  });
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="picker" class:in-sheet={!!sheet} bind:this={root} onkeydown={onKey}>
  <button
    type="button"
    class="current"
    aria-haspopup="listbox"
    aria-expanded={open}
    aria-label="{label}: {current?.label ?? value}"
    onclick={() => (open ? close() : void show())}
  >
    <span class="text">{current?.label ?? value}</span>
    {#if current?.hint}<span class="hint">{current.hint}</span>{/if}
    <svg class="chev" viewBox="0 0 24 24" aria-hidden="true"><path d="M7 10l5 5 5-5" /></svg>
  </button>
  {#if open}
    <div class="pop sheet-{sheet ?? 'none'}" class:up class:sheet={!!sheet} style:width={!sheet && width ? `${width}px` : null}>
      {#if searchable}
        <input class="search" bind:this={search} bind:value={query} placeholder="Search" aria-label="Search {label}" spellcheck="false" />
      {/if}
      <ul role="listbox" aria-label={label} bind:this={list}>
        {#each shown as o, i (o.id)}
          <!-- svelte-ignore a11y_click_events_have_key_events (keys are handled on the list) -->
          <li
            role="option"
            data-i={i}
            aria-selected={o.id === value}
            class:active={i === active}
            onpointerenter={() => (active = i)}
            onpointerdown={(e) => e.preventDefault()}
            onclick={() => pick(o.id)}
          >
            <span class="text">{o.label}</span>
            {#if o.hint}<span class="hint">{o.hint}</span>{/if}
          </li>
        {:else}
          <li class="none">No matches</li>
        {/each}
      </ul>
    </div>
  {/if}
</div>

<style>
  .picker { position: relative; min-width: 0; }
  .current {
    width: 100%; display: flex; align-items: center; gap: 6px;
    height: 32px; padding: 0 8px 0 10px; border: 0; border-radius: 9px;
    background: var(--surface-1); color: var(--fg); font: inherit; font-size: 12.5px; cursor: pointer;
    transition: background 160ms;
  }
  .current:hover, .current[aria-expanded="true"] { background: var(--surface-2); }
  .current:focus { outline: none; }
  .current:focus-visible { outline: 1.5px solid var(--glow, var(--accent)); outline-offset: 1px; }
  .text { flex: 1; min-width: 0; text-align: left; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .hint { color: var(--muted); font-size: 11.5px; flex: none; }
  .chev { width: 14px; height: 14px; flex: none; fill: none; stroke: var(--muted); stroke-width: 2; stroke-linecap: round; stroke-linejoin: round; transition: transform 200ms; }
  [aria-expanded="true"] .chev { transform: rotate(180deg); }
  .pop {
    position: absolute; z-index: 20; top: calc(100% + 4px); right: 0; min-width: 100%;
    padding: 4px; border-radius: 12px;
    background: var(--island-bg); box-shadow: 0 0 0 1px var(--hairline), 0 12px 32px var(--shadow);
    animation: drop 160ms cubic-bezier(0.2, 0.9, 0.25, 1);
    transform-origin: 50% 0;
  }
  .pop.up { top: auto; bottom: calc(100% + 4px); transform-origin: 50% 100%; }
  .picker.in-sheet { position: static; }
  .pop.sheet {
    top: 0; bottom: 0; right: auto; left: 0; width: calc(50% - 24px); min-width: 0;
    display: flex; flex-direction: column; padding: 6px; border-radius: 16px;
  }
  .pop.sheet-right { left: auto; right: 0; }
  .pop.sheet ul { max-height: none; flex: 1; min-height: 0; }
  .search {
    width: 100%; height: 28px; margin-bottom: 4px; padding: 0 8px; border: 0; border-radius: 8px; outline: none;
    background: var(--surface-2); color: var(--fg); font: inherit; font-size: 12px;
  }
  ul { list-style: none; margin: 0; padding: 0; max-height: 148px; overflow-y: auto; scrollbar-width: thin; }
  li {
    display: flex; align-items: center; gap: 8px; padding: 6px 8px; border-radius: 8px;
    font-size: 12.5px; color: var(--fg); cursor: pointer; white-space: nowrap;
  }
  li.active { background: var(--surface-2); }
  li[aria-selected="true"] .text { color: var(--glow, var(--accent)); font-weight: 600; }
  li.none { color: var(--muted); cursor: default; }
  @keyframes drop { from { opacity: 0; transform: translateY(-4px) scale(0.98); } }
  .up { animation-name: rise; }
  @keyframes rise { from { opacity: 0; transform: translateY(4px) scale(0.98); } }
</style>
