<script lang="ts">
  /** Tiny line chart for 0–100 values (oldest first). */
  let { values, width = 120, height = 32 }: { values: number[]; width?: number; height?: number } = $props();
  const points = $derived.by(() => {
    if (values.length < 2) return "";
    const step = width / (values.length - 1);
    return values
      .map((v, i) => `${(i * step).toFixed(1)},${(height - (Math.min(100, Math.max(0, v)) / 100) * (height - 2) - 1).toFixed(1)}`)
      .join(" ");
  });
</script>

<!-- Stretches to the card width; strokes keep their thickness. -->
<svg {height} viewBox="0 0 {width} {height}" preserveAspectRatio="none" aria-hidden="true">
  {#if points}
    <polyline class="area" points="0,{height} {points} {width},{height}" />
    <polyline class="line" {points} />
  {/if}
</svg>

<style>
  svg { display: block; width: 100%; overflow: visible; }
  .line {
    fill: none;
    stroke: var(--glow, var(--accent));
    stroke-width: 1.6;
    stroke-linejoin: round;
    stroke-linecap: round;
    vector-effect: non-scaling-stroke;
  }
  .area {
    fill: var(--glow, var(--accent));
    opacity: 0.12;
  }
</style>
