<script lang="ts">
  import Icon from "./Icon.svelte";
  let { src = null, size = 40, radius = 10 }: { src?: string | null; size?: number; radius?: number } =
    $props();
  let failed = $state(false);
  $effect(() => {
    src;
    failed = false;
  });
</script>

<span class="art" style:width="{size}px" style:height="{size}px" style:border-radius="{radius}px">
  {#if src && !failed}
    {#key src}
      <img {src} alt="" draggable="false" onerror={() => (failed = true)} />
    {/key}
  {:else}
    <Icon name="note" size={Math.round(size * 0.5)} />
  {/if}
</span>

<style>
  .art {
    flex: none;
    display: grid;
    place-items: center;
    overflow: hidden;
    background: var(--surface-2);
    color: var(--muted);
    box-shadow: inset 0 0 0 1px var(--hairline);
  }
  img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    animation: art-in 380ms cubic-bezier(0.2, 0.8, 0.2, 1);
  }
  @keyframes art-in {
    from { opacity: 0; transform: scale(1.08); filter: blur(6px); }
  }
</style>
