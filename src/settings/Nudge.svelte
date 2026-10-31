<script lang="ts">
  /** Left/right arrows that move something a few pixels at a time. Hold to keep moving. */
  let { label, hint = "", value, step = 4, max, disabled = false, onchange }: {
    label: string;
    hint?: string;
    value: number;
    step?: number;
    max: number;
    disabled?: boolean;
    onchange: (value: number) => void;
  } = $props();

  let repeat: ReturnType<typeof setTimeout> | null = null;
  let current = 0;

  function move(by: number) {
    current = Math.max(-max, Math.min(max, current + by));
    onchange(current);
  }
  function press(by: number) {
    current = value;
    move(by);
    // Holding the button keeps moving, faster after a moment.
    let delay = 380;
    const again = () => {
      move(by);
      delay = Math.max(50, delay * 0.7);
      repeat = setTimeout(again, delay);
    };
    repeat = setTimeout(again, delay);
  }
  function release() {
    if (repeat) clearTimeout(repeat);
    repeat = null;
  }
</script>

<div class="row" class:disabled>
  <span class="text">
    <span class="label">{label}</span>
    {#if hint}<span class="hint">{hint}</span>{/if}
  </span>
  <div class="nudge">
    <button
      aria-label="Move left"
      {disabled}
      onpointerdown={() => press(-step)}
      onpointerup={release}
      onpointerleave={release}
      onkeydown={(e) => (e.key === "Enter" || e.key === " ") && (e.preventDefault(), (current = value), move(-step))}
    >
      <svg width="14" height="14" viewBox="0 0 24 24" aria-hidden="true"><path d="M15 5l-7 7 7 7" /></svg>
    </button>
    <output>{value > 0 ? `+${value}` : value} px</output>
    <button
      aria-label="Move right"
      {disabled}
      onpointerdown={() => press(step)}
      onpointerup={release}
      onpointerleave={release}
      onkeydown={(e) => (e.key === "Enter" || e.key === " ") && (e.preventDefault(), (current = value), move(step))}
    >
      <svg width="14" height="14" viewBox="0 0 24 24" aria-hidden="true"><path d="M9 5l7 7-7 7" /></svg>
    </button>
    <button class="reset" onclick={() => onchange(0)} disabled={disabled || value === 0}>Reset</button>
  </div>
</div>

<style>
  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    padding: 12px 0;
  }
  :global(.row) + .row { border-top: 1px solid var(--hairline); }
  .disabled { opacity: 0.5; }
  .text { display: flex; flex-direction: column; gap: 2px; }
  .label { font-weight: 550; }
  .hint { color: var(--muted); font-size: 12.5px; }
  .nudge {
    flex: none;
    display: flex;
    align-items: center;
    gap: 2px;
    padding: 3px;
    border-radius: 10px;
    background: var(--surface-2, var(--surface-1));
    border: 1px solid var(--hairline);
  }
  button {
    height: 28px;
    min-width: 30px;
    display: grid;
    place-items: center;
    padding: 0 8px;
    border: 0;
    border-radius: 7px;
    background: transparent;
    color: var(--fg);
    font: inherit;
    font-size: 12.5px;
    cursor: pointer;
    touch-action: none;
    transition: background 140ms, transform 120ms;
  }
  button:hover:not(:disabled) { background: var(--accent); color: #fff; }
  button:active:not(:disabled) { transform: scale(0.92); }
  button:disabled { cursor: default; opacity: 0.45; }
  .reset { color: var(--muted); }
  svg { fill: none; stroke: currentColor; stroke-width: 2.4; stroke-linecap: round; stroke-linejoin: round; }
  output {
    min-width: 58px;
    text-align: center;
    font-size: 12.5px;
    font-variant-numeric: tabular-nums;
    color: var(--muted);
  }
</style>
