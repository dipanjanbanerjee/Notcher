<script lang="ts">
  import { tick } from "svelte";
  import { flip } from "svelte/animate";
  import Art from "../../components/Art.svelte";
  import Icon, { type IconName } from "../../components/Icon.svelte";
  import { mediaCommand, type NowPlaying } from "../../lib/bridge";
  import {
    ALL_WIDGETS,
    GRID_CELLS,
    addWidget,
    canAdd,
    cellsUsed,
    loadLayout,
    moveWidget,
    placeWidget,
    removeWidget,
    saveLayout,
    type WidgetId,
  } from "../../lib/home-layout";
  import { clips } from "../../lib/stores/clips.svelte";
  import { focus } from "../../lib/stores/focus.svelte";
  import { noteTitle, notes } from "../../lib/stores/notes.svelte";
  import { vitals } from "../../lib/stores/vitals.svelte";
  import { formatSpan, screen } from "../../lib/stores/screentime.svelte";
  import { weather } from "../../lib/stores/weather.svelte";
  import { formatTemp, skyIcon, skyLabel } from "../../lib/weather";
  import { formatClock, formatDate, formatDuration, meridiem } from "../../lib/time";

  let {
    track,
    clock,
    clock24h,
    ongo,
    editing = $bindable(false),
  }: {
    track: NowPlaying | null;
    clock: Date;
    clock24h: boolean;
    ongo: (tab: "clips" | "focus" | "vitals" | "jot" | "weather") => void;
    editing?: boolean;
  } = $props();

  let layout = $state<WidgetId[]>(loadLayout());
  function change(next: WidgetId[]) {
    layout = next;
    saveLayout(next);
  }
  const addable = $derived(ALL_WIDGETS.filter((w) => canAdd(layout, w.id)));
  const freeCells = $derived(GRID_CELLS - cellsUsed(layout));

  const lastClip = $derived(clips.items.find((c) => !c.pinned) ?? clips.items[0] ?? null);
  const memPct = $derived(vitals.current && vitals.current.memTotal > 0 ? (vitals.current.memUsed / vitals.current.memTotal) * 100 : 0);
  const note = $derived(notes.items.find((n) => n.pinned) ?? notes.items[0] ?? null);
  const noteLines = $derived(note ? note.body.split("\n").slice(1).join(" ").trim() : "");

  const label = (id: WidgetId) => ALL_WIDGETS.find((w) => w.id === id)?.label ?? id;

  $effect(() => {
    if (layout.includes("note") && !notes.items.length) void notes.load();
  });
  $effect(() => {
    if (layout.includes("screen")) return screen.watch();
  });
  $effect(() => {
    if (layout.includes("weather")) return weather.watch();
  });
  const topApp = $derived(screen.today?.apps[0] ?? null);

  // ---- drag to reorder (pointer events: HTML drag and drop is off in the island,
  // which takes files from Explorer instead)

  let grid: HTMLDivElement;
  const cells: Record<string, HTMLDivElement> = {};
  let dragId = $state<WidgetId | null>(null);
  let dragX = $state(0);
  let dragY = $state(0);
  let grab = { x: 0, y: 0 };
  let pointer = { x: 0, y: 0 };
  /** The widget we last swapped with; ignored until the pointer leaves it, so two
   *  widgets of different widths don't keep trading places. */
  let lastTarget: WidgetId | null = null;

  /** A cell's place in the grid, ignoring any transform or animation. */
  const home = (el: HTMLElement) => ({ x: el.offsetLeft, y: el.offsetTop, w: el.offsetWidth, h: el.offsetHeight });
  const inside = (r: ReturnType<typeof home>, x: number, y: number) => x >= r.x && x < r.x + r.w && y >= r.y && y < r.y + r.h;

  function startDrag(e: PointerEvent, id: WidgetId) {
    if (!editing || e.button !== 0 || (e.target as HTMLElement).closest("button")) return;
    const el = cells[id];
    const g = grid.getBoundingClientRect();
    pointer = { x: e.clientX - g.left, y: e.clientY - g.top };
    const r = home(el);
    grab = { x: pointer.x - r.x, y: pointer.y - r.y };
    dragId = id;
    dragX = 0;
    dragY = 0;
    lastTarget = null;
    el.setPointerCapture(e.pointerId);
    e.preventDefault();
  }

  async function moveDrag(e: PointerEvent) {
    if (!dragId) return;
    const g = grid.getBoundingClientRect();
    pointer = { x: e.clientX - g.left, y: e.clientY - g.top };
    follow();
    const target = layout.find((id) => id !== dragId && cells[id] && inside(home(cells[id]), pointer.x, pointer.y));
    if (lastTarget && target !== lastTarget) lastTarget = null;
    if (!target || target === lastTarget) return;
    lastTarget = target;
    change(placeWidget(layout, dragId, layout.indexOf(target)));
    await tick();
    follow();
  }

  /** Keeps the dragged widget under the pointer wherever the grid has placed it. */
  function follow() {
    if (!dragId) return;
    const r = home(cells[dragId]);
    dragX = pointer.x - grab.x - r.x;
    dragY = pointer.y - grab.y - r.y;
  }

  function endDrag() {
    dragId = null;
    lastTarget = null;
  }

  function onKey(e: KeyboardEvent, id: WidgetId) {
    if (!editing) return;
    if (e.key === "ArrowLeft" || e.key === "ArrowUp") change(moveWidget(layout, id, -1));
    else if (e.key === "ArrowRight" || e.key === "ArrowDown") change(moveWidget(layout, id, 1));
    else if (e.key === "Delete" || e.key === "Backspace") change(removeWidget(layout, id));
    else return;
    e.preventDefault();
  }
</script>

<div class="home" class:editing class:dragging={dragId !== null} bind:this={grid}>
  {#each layout as id (id)}
    <!-- svelte-ignore a11y_no_noninteractive_tabindex (it is a button while editing) -->
    <div
      class="cell"
      class:wide={id === "media"}
      class:bare={id === "clock"}
      class:lifted={dragId === id}
      bind:this={cells[id]}
      style:transform={dragId === id ? `translate(${dragX}px, ${dragY}px) scale(1.03)` : null}
      animate:flip={{ duration: dragId === id ? 0 : 240, easing: (t) => 1 - Math.pow(1 - t, 3) }}
      onpointerdown={(e) => startDrag(e, id)}
      onpointermove={moveDrag}
      onpointerup={endDrag}
      onpointercancel={endDrag}
      onkeydown={(e) => onKey(e, id)}
      role={editing ? "button" : undefined}
      tabindex={editing ? 0 : undefined}
      aria-label={editing ? `${label(id)}. Drag to move, or use the arrow keys.` : undefined}
    >
      {#if id === "clock"}
        <section class="time">
          <div class="clock">
            <span class="big">{formatClock(clock, clock24h)}</span>
            {#if !clock24h}<span class="ampm">{meridiem(clock)}</span>{/if}
          </div>
          <span class="muted">{formatDate(clock)}</span>
        </section>
      {:else if id === "media"}
        <section class="media tile">
          {#if track}
            <Art src={track.art} size={52} radius={12} />
            <div class="meta">
              <strong>{track.title}</strong>
              <span class="muted">{track.artist}</span>
            </div>
            <div class="controls">
              <button aria-label="Previous" onclick={() => mediaCommand("previous")}><Icon name="previous" size={16} /></button>
              <button aria-label={track.playing ? "Pause" : "Play"} onclick={() => mediaCommand("play_pause")}>
                <Icon name={track.playing ? "pause" : "play"} size={18} />
              </button>
              <button aria-label="Next" onclick={() => mediaCommand("next")}><Icon name="next" size={16} /></button>
            </div>
          {:else}
            <Art size={52} radius={12} />
            <div class="meta"><strong>Nothing playing</strong><span class="muted">Music shows up here</span></div>
          {/if}
        </section>
      {:else if id === "focus"}
        <button class="tile small" onclick={() => ongo("focus")}>
          <Icon name="timer" size={16} />
          {#if focus.active}
            <span class="value mono">{formatDuration(focus.remaining)}</span>
            <span class="muted">{focus.label}{focus.running ? "" : " · paused"}</span>
          {:else}
            <span class="value">Focus</span>
            <span class="muted">Start a session</span>
          {/if}
        </button>
      {:else if id === "system"}
        <button class="tile small" onclick={() => ongo("vitals")}>
          <Icon name="pulse" size={16} />
          {#if vitals.current}
            <span class="value mono">CPU {Math.round(vitals.current.cpu)}%</span>
            <span class="muted">Memory {Math.round(memPct)}%</span>
          {:else}
            <span class="value">System</span>
            <span class="muted">Loading…</span>
          {/if}
        </button>
      {:else if id === "clipboard"}
        <button class="tile small clip" onclick={() => ongo("clips")}>
          <Icon name="clipboard" size={16} />
          {#if lastClip}
            {#if lastClip.kind === "image" && lastClip.image}
              <img src={lastClip.image} alt="" />
            {:else}
              <span class="snippet">{lastClip.text}</span>
            {/if}
          {:else}
            <span class="muted">Copied items appear here</span>
          {/if}
        </button>
      {:else if id === "screen"}
        <button class="tile small" onclick={() => ongo("vitals")}>
          <Icon name="screen" size={16} />
          {#if screen.today}
            <span class="value mono">{formatSpan(screen.today.totalSecs)}</span>
            <span class="muted one-line">{topApp ? `Most in ${topApp.name}` : "Screen time today"}</span>
          {:else}
            <span class="value">Screen time</span>
            <span class="muted">Loading…</span>
          {/if}
        </button>
      {:else if id === "weather"}
        <button class="tile small" onclick={() => ongo("weather")}>
          <Icon name={weather.now ? (skyIcon(weather.now.code, weather.now.isDay) as IconName) : "sun"} size={16} />
          {#if weather.now}
            <span class="value mono">{formatTemp(weather.now.temp, weather.unit)}</span>
            <span class="muted one-line">{weather.now.place} · {skyLabel(weather.now.code)}</span>
          {:else}
            <span class="value">Weather</span>
            <span class="muted one-line">{weather.error ?? "Loading…"}</span>
          {/if}
        </button>
      {:else if id === "note"}
        <button class="tile small" onclick={() => ongo("jot")}>
          <Icon name="pen" size={16} />
          {#if note}
            <span class="value one-line">{noteTitle(note.body)}</span>
            {#if noteLines}<span class="snippet muted">{noteLines}</span>{/if}
          {:else}
            <span class="value">Notes</span>
            <span class="muted">Write something down</span>
          {/if}
        </button>
      {/if}

      {#if editing}
        <div class="edit-bar">
          <span class="grip" aria-hidden="true"><i></i><i></i><i></i><i></i><i></i><i></i></span>
          <span>{label(id)}</span>
          <button class="remove" aria-label="Remove {label(id)}" title="Remove" onclick={() => change(removeWidget(layout, id))}>
            <Icon name="close" size={14} />
          </button>
        </div>
      {/if}
    </div>
  {/each}

  {#if editing && freeCells > 0}
    <div class="cell add" style:grid-column="span {Math.min(freeCells, 4)}">
      {#if addable.length}
        <span class="muted">Add a widget</span>
        <div class="chips">
          {#each addable as w (w.id)}
            <button onclick={() => change(addWidget(layout, w.id))}><Icon name="plus" size={13} />{w.label}</button>
          {/each}
        </div>
      {:else}
        <span class="muted">Remove one to make room</span>
      {/if}
    </div>
  {/if}
</div>

<style>
  .home {
    height: 100%;
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    grid-template-rows: repeat(2, minmax(0, 1fr));
    grid-auto-flow: row dense;
    gap: 10px;
  }
  .cell {
    position: relative;
    min-width: 0;
    min-height: 0;
    display: flex;
  }
  .cell > :global(*:first-child) { flex: 1; min-width: 0; }
  .cell.wide { grid-column: span 2; }
  .editing .cell:not(.add) > :global(*:first-child) {
    pointer-events: none;
    opacity: 0.55;
  }
  .editing .cell:not(.add) {
    border-radius: 16px;
    outline: 1.5px dashed var(--hairline);
    outline-offset: -1px;
    cursor: grab;
    touch-action: none;
    animation: settle 240ms cubic-bezier(0.2, 0.9, 0.25, 1.1);
  }
  .editing .cell:not(.add):hover { outline-color: var(--glow, var(--accent)); }
  .editing .cell:focus-visible { outline: 2px solid var(--glow, var(--accent)); }
  .dragging, .dragging .cell { cursor: grabbing !important; }
  .cell.lifted {
    z-index: 5;
    background: var(--surface-2);
    outline: 1.5px solid var(--glow, var(--accent)) !important;
    box-shadow: 0 14px 34px -10px rgb(0 0 0 / 0.65);
    transition: box-shadow 160ms;
    will-change: transform;
  }
  .cell.lifted > :global(*:first-child) { opacity: 0.9 !important; }
  @keyframes settle { from { transform: scale(0.97); } }

  /* Sits on its own backing so the widget's text never shows through the label. */
  .edit-bar {
    position: absolute;
    inset: auto 6px 6px 6px;
    z-index: 1;
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 3px 3px 3px 8px;
    border-radius: 13px;
    /* The surface colours are see-through, so lay them over the island's own colour. */
    background: linear-gradient(var(--surface-2), var(--surface-2)), var(--island-bg);
    box-shadow: 0 -6px 14px -4px rgb(0 0 0 / 0.45);
    font-size: 12px;
    color: var(--fg);
  }
  .edit-bar span:not(.grip) { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .edit-bar span:not(.grip) { flex: 1; }
  .grip { display: grid; grid-template-columns: repeat(2, 3px); gap: 2px; margin-right: 4px; opacity: 0.6; }
  .grip i { width: 3px; height: 3px; border-radius: 50%; background: currentColor; }
  .edit-bar button,
  .chips button {
    min-width: 26px;
    height: 26px;
    border: 0;
    border-radius: 13px;
    background: var(--surface-2);
    color: var(--fg);
    font: inherit;
    cursor: pointer;
    transition: background 150ms, transform 150ms;
  }
  /* A drawn icon, not a text "×", so it sits in the exact centre of the circle. */
  .edit-bar .remove {
    width: 26px;
    min-width: 0;
    padding: 0;
    display: grid;
    place-items: center;
    flex: none;
  }
  .edit-bar .remove { background: var(--surface-1); }
  .edit-bar button:hover:not(:disabled),
  .chips button:hover { background: var(--glow, var(--accent)); color: #fff; }
  .edit-bar button:active,
  .chips button:active { transform: scale(0.9); }
  .edit-bar button:disabled { opacity: 0.35; cursor: default; }

  .add {
    flex-direction: column;
    justify-content: center;
    gap: 8px;
    padding: 12px;
    border-radius: 16px;
    border: 1.5px dashed var(--hairline);
  }
  .chips { display: flex; flex-wrap: wrap; gap: 6px; }
  .chips button { display: inline-flex; align-items: center; gap: 4px; padding: 0 10px; }

  .time {
    display: flex;
    flex-direction: column;
    justify-content: center;
    padding-left: 4px;
  }
  .clock { display: flex; align-items: baseline; gap: 4px; }
  .big { font-size: 40px; font-weight: 600; letter-spacing: -0.03em; font-variant-numeric: tabular-nums; line-height: 1.1; }
  .ampm { font-size: 13px; color: var(--muted); }
  .muted { color: var(--muted); font-size: 12px; }
  .tile {
    border-radius: 16px;
    background: var(--surface-1);
    box-shadow: inset 0 0 0 1px var(--hairline);
    padding: 12px;
    min-width: 0;
  }
  .media {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .meta { flex: 1; min-width: 0; display: flex; flex-direction: column; }
  .meta strong, .meta span { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .meta strong { font-size: 14px; }
  .controls { display: flex; gap: 2px; }
  .controls button {
    width: 32px; height: 32px; border: 0; border-radius: 50%;
    display: grid; place-items: center;
    background: transparent; color: var(--fg); cursor: pointer;
    transition: background 150ms, transform 150ms;
  }
  .controls button:hover { background: var(--surface-2); }
  .controls button:active { transform: scale(0.88); }
  .small {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 4px;
    border: 0;
    color: var(--fg);
    font: inherit;
    text-align: left;
    cursor: pointer;
    overflow: hidden;
    transition: background 150ms, transform 200ms cubic-bezier(0.2, 0.8, 0.2, 1);
  }
  .small:hover { background: var(--surface-2); }
  .small:active { transform: scale(0.97); }
  .small :global(svg) { color: var(--glow, var(--accent)); }
  .value { font-size: 16px; font-weight: 600; }
  .one-line { max-width: 100%; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .mono { font-variant-numeric: tabular-nums; }
  .snippet {
    font-size: 12.5px;
    display: -webkit-box;
    -webkit-line-clamp: 3;
    line-clamp: 3;
    -webkit-box-orient: vertical;
    overflow: hidden;
    word-break: break-word;
  }
  .clip img { max-width: 100%; max-height: 64px; border-radius: 6px; object-fit: cover; }
</style>
