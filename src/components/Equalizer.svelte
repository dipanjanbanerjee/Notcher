<script lang="ts" module>
  // All equalizers share one slow ticker. The bars jump between levels a few times a
  // second, like a hardware level meter, instead of animating every frame: a CSS
  // animation here kept the whole island repainting 60 times a second.
  const TICK_MS = 120;
  const subscribers = new Set<() => void>();
  let timer: ReturnType<typeof setInterval> | null = null;

  function subscribe(fn: () => void) {
    subscribers.add(fn);
    timer ??= setInterval(() => subscribers.forEach((f) => f()), TICK_MS);
    return () => {
      subscribers.delete(fn);
      if (!subscribers.size && timer) {
        clearInterval(timer);
        timer = null;
      }
    };
  }
</script>

<script lang="ts">
  // Four bars that dance while music plays and settle when paused.
  let { playing = false, size = 14 }: { playing?: boolean; size?: number } = $props();

  const REST = [0.22, 0.22, 0.22, 0.22];
  let levels = $state(REST);
  const reduced = typeof matchMedia === "function" && matchMedia("(prefers-reduced-motion: reduce)").matches;

  $effect(() => {
    if (!playing) {
      levels = REST;
      return;
    }
    if (reduced) {
      levels = [0.6, 0.6, 0.6, 0.6];
      return;
    }
    return subscribe(() => {
      // Each bar drifts towards a new random height, so neighbours stay related.
      levels = levels.map((l) => Math.min(1, Math.max(0.2, l * 0.35 + (0.25 + Math.random() * 0.75) * 0.65)));
    });
  });
</script>

<span class="eq" class:playing style:--size="{size}px" aria-hidden="true">
  {#each levels as l, i (i)}<i style:transform="scaleY({l.toFixed(2)})"></i>{/each}
</span>

<style>
  .eq {
    display: inline-flex;
    align-items: stretch;
    gap: calc(var(--size) * 0.14);
    height: var(--size);
    width: var(--size);
  }
  i {
    flex: 1;
    border-radius: 2px;
    background: var(--glow, var(--accent));
    transform-origin: 50% 100%;
  }
</style>
