<script lang="ts">
  // Four bars that dance while music plays and settle when paused.
  let { playing = false, size = 14 }: { playing?: boolean; size?: number } = $props();
</script>

<span class="eq" class:playing style:--size="{size}px" aria-hidden="true">
  <i></i><i></i><i></i><i></i>
</span>

<style>
  .eq {
    display: inline-flex;
    align-items: flex-end;
    gap: calc(var(--size) * 0.14);
    height: var(--size);
    width: var(--size);
  }
  i {
    flex: 1;
    height: 22%;
    border-radius: 2px;
    background: var(--glow, var(--accent));
    transition: height 260ms cubic-bezier(0.2, 0.8, 0.2, 1);
  }
  .playing i {
    animation: bounce 900ms ease-in-out infinite alternate;
  }
  .playing i:nth-child(2) { animation-duration: 640ms; animation-delay: -200ms; }
  .playing i:nth-child(3) { animation-duration: 780ms; animation-delay: -420ms; }
  .playing i:nth-child(4) { animation-duration: 560ms; animation-delay: -120ms; }
  @keyframes bounce {
    0% { height: 22%; }
    40% { height: 90%; }
    70% { height: 45%; }
    100% { height: 75%; }
  }
  @media (prefers-reduced-motion: reduce) {
    .playing i { animation: none; height: 60%; }
  }
</style>
