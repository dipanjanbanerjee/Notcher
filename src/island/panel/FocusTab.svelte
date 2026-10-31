<script lang="ts">
  import Icon from "../../components/Icon.svelte";
  import type { FocusKind } from "../../lib/focus";
  import { focus } from "../../lib/stores/focus.svelte";
  import { formatDuration } from "../../lib/time";

  const KINDS: { id: FocusKind; label: string }[] = [
    { id: "pomodoro", label: "Pomodoro" },
    { id: "countdown", label: "Timer" },
    { id: "stopwatch", label: "Stopwatch" },
  ];
  const PRESETS = [5, 10, 15, 30, 60];
  const WATER = [0, 30, 45, 60, 90];

  const R = 54;
  const C = 2 * Math.PI * R;
</script>

<div class="focus">
  <div class="dial">
    <svg width="140" height="140" viewBox="0 0 140 140" aria-hidden="true">
      <circle class="track" cx="70" cy="70" r={R} />
      <circle
        class="arc"
        cx="70"
        cy="70"
        r={R}
        stroke-dasharray={C}
        stroke-dashoffset={C * (1 - focus.progress)}
      />
    </svg>
    <div class="readout">
      <span class="time">{formatDuration(focus.remaining)}</span>
      <span class="label">{focus.label}</span>
    </div>
  </div>

  <div class="side">
    <div class="kinds" role="tablist" aria-label="Timer type">
      {#each KINDS as k (k.id)}
        <button role="tab" aria-selected={focus.state.kind === k.id} onclick={() => focus.choose(k.id)}>{k.label}</button>
      {/each}
    </div>

    {#if focus.state.kind === "countdown" && focus.state.phase === "idle"}
      <div class="chips">
        {#each PRESETS as m (m)}
          <button class:on={focus.state.durationMs === m * 60_000} onclick={() => focus.setMinutes(m)}>{m} min</button>
        {/each}
      </div>
    {:else if focus.state.kind === "pomodoro"}
      <div class="sessions" aria-label="{focus.state.completed} of 4 sessions done">
        {#each [0, 1, 2, 3] as i (i)}<i class:done={i < focus.state.completed}></i>{/each}
        <span class="muted">25 min focus · 5 min break</span>
      </div>
    {/if}

    <div class="buttons">
      <button class="primary" onclick={() => focus.toggle()}>
        <Icon name={focus.running ? "pause" : "play"} size={16} />
        {focus.running ? "Pause" : focus.state.phase === "paused" ? "Resume" : "Start"}
      </button>
      <button class="secondary" aria-label="Reset" onclick={() => focus.reset()} disabled={focus.state.phase === "idle" && focus.state.frozenMs === 0}>
        <Icon name="reset" size={16} />
      </button>
    </div>

    <label class="water">
      <Icon name="drop" size={14} />
      <span>Water reminder</span>
      <select value={focus.waterEveryMin} onchange={(e) => focus.setWater(Number(e.currentTarget.value))}>
        {#each WATER as m (m)}<option value={m}>{m === 0 ? "Off" : `Every ${m} min`}</option>{/each}
      </select>
    </label>
  </div>
</div>

<style>
  .focus { height: 100%; display: flex; align-items: center; gap: 28px; padding: 0 12px; }
  .dial { position: relative; width: 140px; height: 140px; flex: none; }
  svg { transform: rotate(-90deg); }
  circle { fill: none; stroke-width: 8; }
  .track { stroke: var(--surface-2); }
  .arc {
    stroke: var(--glow, var(--accent));
    stroke-linecap: round;
    transition: stroke-dashoffset 300ms linear;
  }
  .readout { position: absolute; inset: 0; display: flex; flex-direction: column; align-items: center; justify-content: center; }
  .time { font-size: 28px; font-weight: 600; font-variant-numeric: tabular-nums; letter-spacing: -0.02em; }
  .label { font-size: 12px; color: var(--muted); }
  .side { flex: 1; display: flex; flex-direction: column; gap: 12px; min-width: 0; }
  .kinds { display: inline-flex; padding: 3px; gap: 2px; border-radius: 10px; background: var(--surface-1); align-self: flex-start; }
  .kinds button {
    border: 0; padding: 6px 12px; border-radius: 8px; background: transparent;
    color: var(--muted); font: inherit; font-size: 12.5px; cursor: pointer;
  }
  .kinds button[aria-selected="true"] { background: var(--surface-2); color: var(--fg); }
  .chips { display: flex; gap: 6px; flex-wrap: wrap; }
  .chips button {
    border: 0; padding: 5px 10px; border-radius: 999px; background: var(--surface-1);
    color: var(--muted); font: inherit; font-size: 12px; cursor: pointer;
  }
  .chips button.on { background: var(--glow, var(--accent)); color: #fff; }
  .sessions { display: flex; align-items: center; gap: 6px; }
  .sessions i { width: 10px; height: 10px; border-radius: 50%; background: var(--surface-2); }
  .sessions i.done { background: var(--glow, var(--accent)); }
  .sessions .muted { margin-left: 6px; }
  .muted { color: var(--muted); font-size: 12px; }
  .buttons { display: flex; gap: 8px; }
  .primary {
    display: inline-flex; align-items: center; gap: 6px;
    height: 38px; padding: 0 18px; border: 0; border-radius: 12px;
    background: var(--fg); color: var(--bg); font: inherit; font-weight: 600; cursor: pointer;
    transition: transform 160ms cubic-bezier(0.2, 0.8, 0.2, 1);
  }
  .primary:active { transform: scale(0.95); }
  .secondary {
    width: 38px; height: 38px; display: grid; place-items: center;
    border: 0; border-radius: 12px; background: var(--surface-2); color: var(--fg); cursor: pointer;
  }
  .secondary:disabled { opacity: 0.4; cursor: default; }
  .water { display: flex; align-items: center; gap: 6px; color: var(--muted); font-size: 12.5px; }
  .water :global(svg) { color: var(--glow, var(--accent)); }
  select {
    margin-left: auto; font: inherit; font-size: 12.5px; color: var(--fg);
    background: var(--surface-1); border: 1px solid var(--hairline); border-radius: 8px; padding: 3px 6px;
  }
</style>
