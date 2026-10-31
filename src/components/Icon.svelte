<script lang="ts">
  // Small hand-drawn icon set (24×24 grid).
  const PATHS = {
    play: "M8 5.5v13a1 1 0 0 0 1.5.86l10.4-6.5a1 1 0 0 0 0-1.72L9.5 4.64A1 1 0 0 0 8 5.5z",
    pause: "M7 5h3.2v14H7zM13.8 5H17v14h-3.2z",
    next: "M5 6.2v11.6a.8.8 0 0 0 1.24.66L14.5 13v5h2.5V6h-2.5v5L6.24 5.54A.8.8 0 0 0 5 6.2z",
    previous: "M19 6.2v11.6a.8.8 0 0 1-1.24.66L9.5 13v5H7V6h2.5v5l8.26-5.46A.8.8 0 0 1 19 6.2z",
    gear: "M12 8.6a3.4 3.4 0 1 0 0 6.8 3.4 3.4 0 0 0 0-6.8zm8.2 4.7-1.7-.3a6.8 6.8 0 0 1-.6 1.5l1 1.4-1.6 1.6-1.4-1a6.8 6.8 0 0 1-1.5.6l-.3 1.7h-2.2l-.3-1.7a6.8 6.8 0 0 1-1.5-.6l-1.4 1-1.6-1.6 1-1.4a6.8 6.8 0 0 1-.6-1.5l-1.7-.3v-2.2l1.7-.3c.1-.5.3-1 .6-1.5l-1-1.4 1.6-1.6 1.4 1c.5-.3 1-.5 1.5-.6l.3-1.7h2.2l.3 1.7c.5.1 1 .3 1.5.6l1.4-1 1.6 1.6-1 1.4c.3.5.5 1 .6 1.5l1.7.3z",
    note: "M9 17.5V6.2l10-2.2v11.3M9 17.5a2.5 2.5 0 1 1-5 0 2.5 2.5 0 0 1 5 0zm10-2.2a2.5 2.5 0 1 1-5 0 2.5 2.5 0 0 1 5 0z",
  } as const;

  let { name, size = 18, filled = true }: { name: keyof typeof PATHS; size?: number; filled?: boolean } =
    $props();
</script>

<svg
  width={size}
  height={size}
  viewBox="0 0 24 24"
  aria-hidden="true"
  fill={filled && name !== "note" ? "currentColor" : "none"}
  stroke={filled && name !== "note" ? "none" : "currentColor"}
  stroke-width="1.8"
  stroke-linecap="round"
  stroke-linejoin="round"
>
  <path d={PATHS[name]} fill-rule="evenodd" />
</svg>
