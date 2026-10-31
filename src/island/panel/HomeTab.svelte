<script lang="ts">
  import Art from "../../components/Art.svelte";
  import Icon from "../../components/Icon.svelte";
  import { mediaCommand, type NowPlaying } from "../../lib/bridge";
  import { clips } from "../../lib/stores/clips.svelte";
  import { focus } from "../../lib/stores/focus.svelte";
  import { vitals } from "../../lib/stores/vitals.svelte";
  import { formatClock, formatDate, formatDuration, meridiem } from "../../lib/time";

  let { track, clock, clock24h, ongo }: {
    track: NowPlaying | null;
    clock: Date;
    clock24h: boolean;
    ongo: (tab: "clips" | "focus" | "vitals") => void;
  } = $props();

  const lastClip = $derived(clips.items.find((c) => !c.pinned) ?? clips.items[0] ?? null);
  const memPct = $derived(vitals.current && vitals.current.memTotal > 0 ? (vitals.current.memUsed / vitals.current.memTotal) * 100 : 0);
</script>

<div class="home">
  <section class="time">
    <div class="clock">
      <span class="big">{formatClock(clock, clock24h)}</span>
      {#if !clock24h}<span class="ampm">{meridiem(clock)}</span>{/if}
    </div>
    <span class="muted">{formatDate(clock)}</span>
  </section>

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
</div>

<style>
  .home {
    height: 100%;
    display: grid;
    grid-template-columns: 1fr 1fr 1fr;
    grid-template-rows: auto 1fr;
    gap: 10px;
  }
  .time {
    grid-column: 1;
    display: flex;
    flex-direction: column;
    justify-content: center;
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
    grid-column: 2 / 4;
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
    transition: background 150ms, transform 200ms cubic-bezier(0.2, 0.8, 0.2, 1);
  }
  .small:hover { background: var(--surface-2); }
  .small:active { transform: scale(0.97); }
  .small :global(svg) { color: var(--glow, var(--accent)); }
  .value { font-size: 16px; font-weight: 600; }
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
