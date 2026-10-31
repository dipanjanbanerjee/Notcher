<script lang="ts">
  import { onMount } from "svelte";
  import Art from "../components/Art.svelte";
  import Equalizer from "../components/Equalizer.svelte";
  import Icon from "../components/Icon.svelte";
  import NowPlayingCard from "../components/NowPlayingCard.svelte";
  import {
    focusIsland,
    inTauri,
    nowPlaying,
    onMedia,
    onNativePointer,
    onSettingsChanged,
    onSuppressed,
    onToggle,
    openSettings,
    setHitRegion,
    type NowPlaying,
  } from "../lib/bridge";
  import { colorFromImage, toCss, type RGB } from "../lib/color";
  import {
    GEOMETRY,
    REST_HOT_ZONE,
    TIMING,
    transition,
    type IslandEvent,
    type IslandMode,
  } from "../lib/island-state";
  import {
    inflate,
    islandBounds,
    islandPath,
    sameRect,
    snapOut,
    union,
    type IslandGeometry,
    type Rect,
  } from "../lib/shape";
  import { DEFAULT_SETTINGS, applyTheme, loadSettings, type Settings } from "../lib/settings";
  import { SPRINGS, SpringGroup } from "../lib/spring";
  import { estimatePosition, formatClock, formatDate, meridiem } from "../lib/time";

  /** Must match WINDOW_WIDTH in src-tauri/src/island.rs. */
  const WINDOW_WIDTH = 760;
  const CENTER_X = WINDOW_WIDTH / 2;
  /** Content swaps once the shape is this far through its motion. */
  const CONTENT_SWAP_AT = 0.6;

  const reducedMotion = matchMedia("(prefers-reduced-motion: reduce)");

  let settings = $state<Settings>({ ...DEFAULT_SETTINGS });
  let mode = $state<IslandMode>("rest");
  /** The mode whose content is currently drawn; lags `mode` until the shape has moved enough. */
  let shownMode = $state<IslandMode>("rest");
  let contentVisible = $state(true);
  let geometry = $state<IslandGeometry>({ ...GEOMETRY.rest });
  let hovered = $state(false);
  let suppressed = $state(false);

  let track = $state<NowPlaying | null>(null);
  let reportedAt = $state(0);
  let now = $state(Date.now());
  let glowColor = $state<RGB | null>(null);

  const positionMs = $derived(
    track ? estimatePosition(track.positionMs, reportedAt, now, track.playing, track.durationMs) : 0,
  );
  const clock = $derived(new Date(now));
  const path = $derived(islandPath(geometry, CENTER_X));
  const body = $derived(islandBounds({ ...geometry, shoulder: 0 }, CENTER_X));
  const glowOn = $derived(settings.glow && !!track?.playing && !suppressed);
  const glowCss = $derived(glowColor ? toCss(glowColor) : null);

  // ---------------------------------------------------------------- motion

  let swapFrom: IslandGeometry = { ...GEOMETRY.rest };
  let lastRegion: Rect | null = null;

  const springs = new SpringGroup(
    { width: GEOMETRY.rest.width, height: GEOMETRY.rest.height, radius: GEOMETRY.rest.radius, shoulder: GEOMETRY.rest.shoulder },
    (v, settled) => {
      geometry = v;
      maybeSwapContent(v);
      // The region already covers the whole motion (see animateTo), so it is only
      // tightened once the shape comes to rest. No per-frame IPC keeps frames smooth.
      if (settled) reportHitRegion(v);
    },
  );

  function progressToward(target: IslandGeometry, v: IslandGeometry): number {
    const span = Math.max(Math.abs(target.width - swapFrom.width), Math.abs(target.height - swapFrom.height));
    if (span < 1) return 1;
    const left = Math.max(Math.abs(target.width - v.width), Math.abs(target.height - v.height));
    return 1 - left / span;
  }

  function maybeSwapContent(v: IslandGeometry) {
    if (shownMode === mode) return;
    if (progressToward(GEOMETRY[mode], v) >= CONTENT_SWAP_AT) {
      shownMode = mode;
      contentVisible = true;
    }
  }

  function reportHitRegion(v: IslandGeometry) {
    let region: Rect;
    if (suppressed) {
      region = { x: 0, y: 0, width: 0, height: 0 };
    } else {
      // Cover both where the island is and where it is going, so input never lags the shape.
      region = union(islandBounds(v, CENTER_X), islandBounds(GEOMETRY[mode], CENTER_X));
      if (mode === "rest") region = inflate(region, REST_HOT_ZONE.sides, REST_HOT_ZONE.bottom);
    }
    region = snapOut(region);
    if (!sameRect(lastRegion, region)) {
      lastRegion = region;
      void setHitRegion(region);
    }
  }

  function animateTo(next: IslandMode) {
    swapFrom = { ...geometry };
    if (next !== shownMode) contentVisible = false;
    springs.to(GEOMETRY[next], next === "rest" ? SPRINGS.close : SPRINGS.shape, reducedMotion.matches);
    // The listener runs on the next frame; update the region now so expanding never blocks input.
    reportHitRegion(geometry);
  }

  // ---------------------------------------------------------------- state machine

  const timers = new Map<string, ReturnType<typeof setTimeout>>();
  function later(name: string, ms: number, fn: () => void) {
    clearTimeout(timers.get(name));
    timers.set(name, setTimeout(() => {
      timers.delete(name);
      fn();
    }, ms));
  }
  function cancel(...names: string[]) {
    for (const n of names) {
      clearTimeout(timers.get(n));
      timers.delete(n);
    }
  }

  function dispatch(event: IslandEvent) {
    const next = transition(mode, event, { hoverToOpen: settings.hoverToOpen });
    if (next === mode) return;
    const prev = mode;
    mode = next;
    if (next !== "live") cancel("live");
    if (next === "peek" && hovered && settings.hoverToOpen) later("dwell", TIMING.dwellToOpen, () => dispatch({ type: "dwell" }));
    if (next !== "peek") cancel("dwell");
    if (next === "open" && prev !== "open") now = Date.now();
    animateTo(next);
  }

  function pointerEnter() {
    if (hovered) return;
    hovered = true;
    cancel("leave");
    if (mode === "rest") later("intent", TIMING.hoverIntent, () => dispatch({ type: "hoverIntent" }));
    else if (mode === "peek" && settings.hoverToOpen) later("dwell", TIMING.dwellToOpen, () => dispatch({ type: "dwell" }));
  }

  function pointerLeave() {
    if (!hovered) return;
    hovered = false;
    cancel("intent", "dwell");
    later("leave", TIMING.leaveGrace, () => dispatch({ type: "leave" }));
  }

  function toggle() {
    dispatch({ type: "toggle" });
    if (mode === "open") void focusIsland();
  }

  function showLiveActivity() {
    if (mode !== "rest" && mode !== "live") return;
    dispatch({ type: "liveStart" });
    later("live", TIMING.liveDuration, () => dispatch({ type: "liveEnd", hovered }));
  }

  // ---------------------------------------------------------------- media

  function receiveTrack(next: NowPlaying | null, announce: boolean) {
    const changed =
      !!next && (!track || track.title !== next.title || track.artist !== next.artist);
    if (next?.art !== track?.art) {
      glowColor = null;
      if (next?.art) {
        const src = next.art;
        void colorFromImage(src).then((c) => {
          if (track?.art === src) glowColor = c;
        });
      }
    }
    track = next;
    reportedAt = Date.now();
    if (announce && changed && next?.playing && settings.trackChangeActivity) showLiveActivity();
  }

  // ---------------------------------------------------------------- browser preview

  /**
   * `npm run dev` opened in a normal browser has no Rust side. Add `?demo` to the URL to
   * get a wallpaper and a fake track so the island can be designed and tested there.
   */
  function startBrowserPreview() {
    const params = new URLSearchParams(location.search);
    if (!params.has("demo")) return;
    document.documentElement.classList.add("preview");
    const art =
      "data:image/svg+xml;utf8," +
      encodeURIComponent(
        `<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64"><defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="#ff5f6d"/><stop offset="1" stop-color="#7b2ff7"/></linearGradient></defs><rect width="64" height="64" fill="url(#g)"/><circle cx="44" cy="22" r="10" fill="#ffd36e"/></svg>`,
      );
    receiveTrack(
      { title: "Midnight Static", artist: "Glass Harbor", album: "Low Tide", art, playing: true, positionMs: 42_000, durationMs: 214_000, source: "Preview" },
      false,
    );
    const m = params.get("mode");
    if (m === "open" || m === "peek" || m === "live") {
      mode = m;
      shownMode = m;
      springs.to(GEOMETRY[m], undefined, true);
    }
    (window as unknown as { notcher: object }).notcher = {
      dispatch,
      nextTrack: () =>
        receiveTrack({ ...track!, title: "Paper Satellites", artist: "Low Orbit Club", positionMs: 0 }, true),
    };
  }

  // ---------------------------------------------------------------- lifecycle

  onMount(() => {
    const unlisten: Promise<() => void>[] = [];
    void loadSettings().then((s) => {
      settings = s;
      applyTheme(s.theme);
    });
    if (inTauri) void nowPlaying().then((np) => receiveTrack(np, false));
    else startBrowserPreview();

    unlisten.push(onMedia((np) => receiveTrack(np, true)));
    unlisten.push(onToggle(toggle));
    unlisten.push(
      onSuppressed(({ value }) => {
        suppressed = value;
        if (value) dispatch({ type: "suppress" });
        reportHitRegion(geometry);
      }),
    );
    // Windows reports enter/leave from the native side (the web view is click-through until then).
    unlisten.push(onNativePointer(({ value }) => (value ? pointerEnter() : pointerLeave())));
    unlisten.push(
      onSettingsChanged(() =>
        void loadSettings().then((s) => {
          settings = s;
          applyTheme(s.theme);
        }),
      ),
    );

    const tick = setInterval(() => (now = Date.now()), 250);
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") dispatch({ type: "escape" });
    };
    const onBlur = () => {
      if (mode === "open" && !hovered) dispatch({ type: "leave" });
    };
    // The OS only delivers pointer input inside the hit region, so the pointer being
    // anywhere in that region means it is on the island (including the hover margin).
    const onMove = (e: PointerEvent) => {
      const r = lastRegion;
      const inside = !!r && e.clientX >= r.x && e.clientX < r.x + r.width && e.clientY >= r.y && e.clientY < r.y + r.height;
      if (inside) pointerEnter();
      else pointerLeave();
    };
    const onOut = () => pointerLeave();
    document.addEventListener("pointermove", onMove);
    document.documentElement.addEventListener("pointerleave", onOut);
    window.addEventListener("keydown", onKey);
    window.addEventListener("blur", onBlur);
    reportHitRegion(geometry);

    return () => {
      clearInterval(tick);
      document.removeEventListener("pointermove", onMove);
      document.documentElement.removeEventListener("pointerleave", onOut);
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("blur", onBlur);
      springs.stop();
      for (const t of timers.values()) clearTimeout(t);
      for (const u of unlisten) void u.then((f) => f());
    };
  });
</script>

<div class="stage" class:suppressed style:--glow={glowCss}>
  <!-- Shadow and glow use box-shadow on a plain box: far cheaper to repaint every
       frame than SVG filters, especially in WebKitGTK. -->
  <div
    class="halo"
    style:left="{body.x}px"
    style:width="{body.width}px"
    style:height="{body.height}px"
    style:border-radius="0 0 {geometry.radius * 1.15}px {geometry.radius * 1.15}px"
  >
    <div class="shadow"></div>
    <div class="glow" class:on={glowOn} class:breathe={glowOn && mode === "rest"}></div>
  </div>
  <svg class="shape" width={WINDOW_WIDTH} height="400" aria-hidden="true">
    <path class="fill" d={path} />
    <path class="rim" d={path} />
  </svg>

  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="island"
    style:left="{body.x}px"
    style:width="{body.width}px"
    style:height="{body.height}px"
    style:border-bottom-left-radius="{geometry.radius * 1.15}px"
    style:border-bottom-right-radius="{geometry.radius * 1.15}px"
    onclick={() => dispatch({ type: "click" })}
    onkeydown={(e) => e.key === "Enter" && dispatch({ type: "click" })}
    role="button"
    tabindex="-1"
    aria-label="Notcher island"
    aria-expanded={mode === "open"}
  >
    <div class="content mode-{shownMode}" class:visible={contentVisible}>
      {#if shownMode === "rest"}
        {#if track}
          <div class="rest-row">
            <Art src={track.art} size={20} radius={6} />
            <Equalizer playing={track.playing} size={14} />
          </div>
        {/if}
      {:else if shownMode === "peek"}
        <div class="peek-row">
          {#if track}
            <Art src={track.art} size={28} radius={8} />
            <div class="line">
              <strong>{track.title}</strong>
              <span>{track.artist}</span>
            </div>
            <Equalizer playing={track.playing} size={16} />
          {:else}
            <strong class="clock-sm">{formatClock(clock, settings.clock24h)}</strong>
            <span class="muted">{formatDate(clock)}</span>
          {/if}
        </div>
      {:else if shownMode === "live" && track}
        <div class="live-row">
          <Art src={track.art} size={54} radius={14} />
          <div class="stack">
            <span class="eyebrow">Now playing</span>
            <strong>{track.title}</strong>
            <span class="muted">{track.artist}</span>
          </div>
          <Equalizer playing={track.playing} size={20} />
        </div>
      {:else if shownMode === "open"}
        <div class="open-panel">
          <header>
            <div class="time">
              <span class="clock">{formatClock(clock, settings.clock24h)}</span>
              {#if !settings.clock24h}<span class="ampm">{meridiem(clock)}</span>{/if}
            </div>
            <span class="muted date">{formatDate(clock)}</span>
            <button class="gear" aria-label="Settings" onclick={(e) => { e.stopPropagation(); void openSettings(); }}>
              <Icon name="gear" size={18} />
            </button>
          </header>
          <NowPlayingCard {track} {positionMs} />
        </div>
      {/if}
    </div>
  </div>
</div>

<style>
  .stage {
    position: fixed;
    inset: 0;
    pointer-events: none;
    transition: opacity 220ms ease, transform 320ms cubic-bezier(0.2, 0.8, 0.2, 1);
  }
  .stage.suppressed {
    opacity: 0;
    transform: translateY(-12px);
  }
  .shape {
    position: absolute;
    inset: 0;
    overflow: visible;
  }
  .fill {
    fill: var(--island-bg);
  }
  .rim {
    fill: none;
    stroke: var(--rim);
    stroke-width: 1;
  }
  .halo {
    position: absolute;
    top: 0;
  }
  .halo > div {
    position: absolute;
    inset: 0;
    border-radius: inherit;
  }
  .shadow {
    box-shadow: 0 10px 28px var(--shadow), 0 2px 6px var(--shadow);
  }
  .glow {
    opacity: 0;
    box-shadow: 0 0 18px 3px var(--glow, var(--accent));
    transition: opacity 500ms ease;
    will-change: opacity;
  }
  .glow.on {
    opacity: 0.75;
  }
  .glow.breathe {
    animation: breathe 3.6s ease-in-out infinite;
  }
  @keyframes breathe {
    0%, 100% { opacity: 0.3; }
    50% { opacity: 0.75; }
  }

  .island {
    position: absolute;
    top: 0;
    overflow: hidden;
    pointer-events: auto;
    color: var(--fg);
    cursor: default;
    outline: none;
  }
  .content {
    position: absolute;
    inset: 0;
    opacity: 0;
    transform: scale(0.96) translateY(-4px);
    transform-origin: 50% 0;
    transition:
      opacity 90ms ease,
      transform 90ms ease;
    will-change: opacity, transform;
  }
  .content.visible {
    opacity: 1;
    transform: none;
    transition:
      opacity 200ms ease,
      transform 320ms cubic-bezier(0.2, 0.9, 0.25, 1.1);
  }

  .rest-row {
    height: 100%;
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 12px 0 10px;
  }
  .peek-row {
    height: 100%;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 16px 0 10px;
  }
  .peek-row .line {
    flex: 1;
    min-width: 0;
    display: flex;
    gap: 8px;
    align-items: baseline;
    white-space: nowrap;
    overflow: hidden;
  }
  .peek-row .line strong {
    font-size: 13px;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .peek-row .line span {
    font-size: 12px;
    color: var(--muted);
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .clock-sm {
    font-size: 14px;
    font-variant-numeric: tabular-nums;
    padding-left: 8px;
  }
  .live-row {
    height: 100%;
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 0 18px 0 12px;
  }
  .stack {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  .stack strong,
  .stack .muted {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .stack strong { font-size: 14px; }
  .stack .muted { font-size: 12px; }
  .eyebrow {
    font-size: 10px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--glow, var(--accent));
  }
  .muted { color: var(--muted); }

  .open-panel {
    height: 100%;
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding: 18px 22px 20px;
  }
  header {
    display: flex;
    align-items: baseline;
    gap: 12px;
  }
  .time { display: flex; align-items: baseline; gap: 4px; }
  .clock {
    font-size: 34px;
    font-weight: 600;
    letter-spacing: -0.02em;
    font-variant-numeric: tabular-nums;
  }
  .ampm { font-size: 13px; color: var(--muted); }
  .date { flex: 1; font-size: 13px; }
  .gear {
    align-self: center;
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    border: 0;
    border-radius: 50%;
    background: transparent;
    color: var(--muted);
    cursor: pointer;
    transition: background 160ms, color 160ms, transform 400ms cubic-bezier(0.2, 0.8, 0.2, 1);
  }
  .gear:hover { background: var(--surface-2); color: var(--fg); transform: rotate(60deg); }
  .gear:focus-visible { outline: 2px solid var(--accent); }

  @media (prefers-reduced-motion: reduce) {
    .content, .content.visible { transform: none; }
    .glow.breathe { animation: none; opacity: 0.6; }
  }
</style>
