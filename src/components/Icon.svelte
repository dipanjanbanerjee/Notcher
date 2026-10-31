<script lang="ts" module>
  // Hand-drawn icon set on a 24×24 grid. "Solid" icons are filled; the rest are strokes.
  const SOLID = {
    play: "M8 5.5v13a1 1 0 0 0 1.5.86l10.4-6.5a1 1 0 0 0 0-1.72L9.5 4.64A1 1 0 0 0 8 5.5z",
    pause: "M7 5h3.2v14H7zM13.8 5H17v14h-3.2z",
    next: "M5 6.2v11.6a.8.8 0 0 0 1.24.66L14.5 13v5h2.5V6h-2.5v5L6.24 5.54A.8.8 0 0 0 5 6.2z",
    previous: "M19 6.2v11.6a.8.8 0 0 1-1.24.66L9.5 13v5H7V6h2.5v5l8.26-5.46A.8.8 0 0 1 19 6.2z",
    gear: "M12 8.6a3.4 3.4 0 1 0 0 6.8 3.4 3.4 0 0 0 0-6.8zm8.2 4.7-1.7-.3a6.8 6.8 0 0 1-.6 1.5l1 1.4-1.6 1.6-1.4-1a6.8 6.8 0 0 1-1.5.6l-.3 1.7h-2.2l-.3-1.7a6.8 6.8 0 0 1-1.5-.6l-1.4 1-1.6-1.6 1-1.4a6.8 6.8 0 0 1-.6-1.5l-1.7-.3v-2.2l1.7-.3c.1-.5.3-1 .6-1.5l-1-1.4 1.6-1.6 1.4 1c.5-.3 1-.5 1.5-.6l.3-1.7h2.2l.3 1.7c.5.1 1 .3 1.5.6l1.4-1 1.6 1.6-1 1.4c.3.5.5 1 .6 1.5l1.7.3z",
  } as const;
  const STROKE = {
    note: "M9 17.5V6.2l10-2.2v11.3M9 17.5a2.5 2.5 0 1 1-5 0 2.5 2.5 0 0 1 5 0zm10-2.2a2.5 2.5 0 1 1-5 0 2.5 2.5 0 0 1 5 0z",
    home: "M4 11.2 12 4.5l8 6.7M6.5 9.4V19h11V9.4M10 19v-5h4v5",
    clipboard: "M9 4.5h6v2.5H9zM9 5.5H6.5v14h11v-14H15M9 11h6M9 14.5h4",
    pen: "M4.5 19.5l1-4L15.8 5.2a2 2 0 0 1 2.9 0l.1.1a2 2 0 0 1 0 2.9L8.5 18.5zM13.8 7.2l3 3",
    timer: "M12 21a7.5 7.5 0 1 0 0-15 7.5 7.5 0 0 0 0 15zM12 9.5v4l2.5 1.5M9.5 3h5",
    pulse: "M3 12h4l2-5 4 10 2-5h6",
    pin: "M9 4h6l-1 6 3 3v1.5H7V13l3-3zM12 14.5V20",
    trash: "M5 7h14M10 7V4.5h4V7M7 7l1 12.5h8L17 7M10.5 11v5M13.5 11v5",
    search: "M10.5 17a6.5 6.5 0 1 0 0-13 6.5 6.5 0 0 0 0 13zM15.5 15.5 20 20",
    plus: "M12 5v14M5 12h14",
    check: "M5 12.5l4.5 4.5L19 7.5",
    reset: "M5 12a7 7 0 1 0 2-4.9M5 4v4h4",
    drop: "M12 3.5c3 4 5.5 7 5.5 10a5.5 5.5 0 0 1-11 0c0-3 2.5-6 5.5-10z",
    bolt: "M13 3 5.5 13.5H12L11 21l7.5-10.5H12z",
    copy: "M8.5 8.5h10v11h-10zM15.5 8.5V4.5h-10v11h3",
    bell: "M6.5 16.5v-5a5.5 5.5 0 0 1 11 0v5l1.5 2h-14zM10 20.5a2 2 0 0 0 4 0",
    beam: "M3.5 7.5h7v12h-7zM6 17h2M13.5 4.5h7v10h-7zM16 12.5h2M10.5 11.5h3M12 10l1.5 1.5L12 13",
    folder: "M3.5 6.5a1.5 1.5 0 0 1 1.5-1.5h4l2 2h8a1.5 1.5 0 0 1 1.5 1.5v9a1.5 1.5 0 0 1-1.5 1.5H5a1.5 1.5 0 0 1-1.5-1.5z",
    upload: "M12 15.5V4.5M7.5 9 12 4.5 16.5 9M5 14.5v3a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2v-3",
    image: "M4.5 5.5h15v13h-15zM4.5 15.5l4.5-4.5 4 4 2.5-2.5 4 4M15 9.5h.01",
    film: "M4.5 5.5h15v13h-15zM8 5.5v13M16 5.5v13M4.5 9.5H8M4.5 14.5H8M16 9.5h3.5M16 14.5h3.5",
    doc: "M7 3.5h7l4.5 4.5v12.5H7zM14 3.5V8h4.5M9.5 12.5h6M9.5 16h6",
    box: "M4.5 7.5 12 4l7.5 3.5v9L12 20l-7.5-3.5zM4.5 7.5 12 11l7.5-3.5M12 11v9",
    arrowDown: "M12 5v14M6.5 13.5 12 19l5.5-5.5",
    arrowUp: "M12 19V5M6.5 10.5 12 5l5.5 5.5",
    stop: "M7 7h10v10H7z",
    external: "M14 4.5h5.5V10M19.5 4.5 11 13M17 13.5v5a1 1 0 0 1-1 1H5.5a1 1 0 0 1-1-1V8a1 1 0 0 1 1-1h5",
    close: "M6.5 6.5l11 11M17.5 6.5l-11 11",
    screen: "M3.5 5h17v11h-17zM9 20h6M12 16v4",
    phone: "M8 3.5h8a1.5 1.5 0 0 1 1.5 1.5v14a1.5 1.5 0 0 1-1.5 1.5H8A1.5 1.5 0 0 1 6.5 19V5A1.5 1.5 0 0 1 8 3.5zM11 17.5h2",
  } as const;
  export type IconName = keyof typeof SOLID | keyof typeof STROKE;
</script>

<script lang="ts">
  let { name, size = 18 }: { name: IconName; size?: number } = $props();
  const solid = $derived(name in SOLID);
  const d = $derived(solid ? SOLID[name as keyof typeof SOLID] : STROKE[name as keyof typeof STROKE]);
</script>

<svg
  width={size}
  height={size}
  viewBox="0 0 24 24"
  aria-hidden="true"
  fill={solid ? "currentColor" : "none"}
  stroke={solid ? "none" : "currentColor"}
  stroke-width="1.8"
  stroke-linecap="round"
  stroke-linejoin="round"
>
  <path {d} fill-rule="evenodd" />
</svg>
