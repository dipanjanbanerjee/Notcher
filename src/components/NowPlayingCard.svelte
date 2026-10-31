<script lang="ts">
  import Art from "./Art.svelte";
  import Icon from "./Icon.svelte";
  import { mediaCommand, type MediaAction, type NowPlaying } from "../lib/bridge";
  import { formatDuration } from "../lib/time";

  let { track, positionMs, onaction }: {
    track: NowPlaying | null;
    positionMs: number;
    onaction?: (action: MediaAction) => void;
  } = $props();

  const progress = $derived(
    track && track.durationMs > 0 ? Math.min(1, Math.max(0, positionMs / track.durationMs)) : 0,
  );

  function act(action: MediaAction) {
    onaction?.(action);
    void mediaCommand(action);
  }
</script>

<section class="card" aria-label="Now playing">
  {#if track}
    <Art src={track.art} size={112} radius={18} />
    <div class="body">
      <div class="meta">
        <span class="source">{track.source || "Now playing"}</span>
        <h2 title={track.title}>{track.title}</h2>
        <p title={track.artist}>{track.artist || track.album}</p>
      </div>
      {#if track.durationMs > 0}
        <div class="timeline">
          <div class="bar"><div class="fill" style:transform="scaleX({progress})"></div></div>
          <div class="times">
            <span>{formatDuration(positionMs)}</span>
            <span>-{formatDuration(Math.max(0, track.durationMs - positionMs))}</span>
          </div>
        </div>
      {/if}
      <div class="controls">
        <button aria-label="Previous track" onclick={() => act("previous")}><Icon name="previous" size={20} /></button>
        <button class="main" aria-label={track.playing ? "Pause" : "Play"} onclick={() => act("play_pause")}>
          <Icon name={track.playing ? "pause" : "play"} size={24} />
        </button>
        <button aria-label="Next track" onclick={() => act("next")}><Icon name="next" size={20} /></button>
      </div>
    </div>
  {:else}
    <Art size={112} radius={18} />
    <div class="body empty">
      <h2>Nothing playing</h2>
      <p>Play something in Spotify, your browser or any media app and it will show up here.</p>
    </div>
  {/if}
</section>

<style>
  .card {
    display: flex;
    gap: 18px;
    align-items: stretch;
    padding: 14px;
    border-radius: 22px;
    background: var(--surface-1);
    box-shadow: inset 0 0 0 1px var(--hairline);
    min-width: 0;
  }
  .body {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    gap: 8px;
  }
  .meta { min-width: 0; }
  .source {
    font-size: 11px;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--glow, var(--accent));
  }
  h2 {
    margin: 2px 0 0;
    font-size: 17px;
    font-weight: 650;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  p {
    margin: 2px 0 0;
    color: var(--muted);
    font-size: 13px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .empty { justify-content: center; }
  .empty p { white-space: normal; }
  .bar {
    height: 4px;
    border-radius: 2px;
    background: var(--surface-2);
    overflow: hidden;
  }
  .fill {
    height: 100%;
    background: var(--glow, var(--accent));
    transform-origin: left;
    transition: transform 260ms linear;
  }
  .times {
    display: flex;
    justify-content: space-between;
    margin-top: 4px;
    font-size: 11px;
    color: var(--muted);
    font-variant-numeric: tabular-nums;
  }
  .controls {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  button {
    display: grid;
    place-items: center;
    width: 36px;
    height: 36px;
    border: 0;
    border-radius: 50%;
    color: var(--fg);
    background: transparent;
    cursor: pointer;
    transition: background 160ms, transform 160ms cubic-bezier(0.2, 0.8, 0.2, 1);
  }
  button:hover { background: var(--surface-2); }
  button:active { transform: scale(0.88); }
  button:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
  .main {
    width: 44px;
    height: 44px;
    background: var(--fg);
    color: var(--bg);
  }
  .main:hover { background: var(--fg); transform: scale(1.05); }
</style>
