<script lang="ts">
  import { onMount, untrack } from "svelte";
  import Art from "../components/Art.svelte";
  import Equalizer from "../components/Equalizer.svelte";
  import Icon from "../components/Icon.svelte";
  import OpenPanel, { TAB_IDS, type TabId } from "./panel/OpenPanel.svelte";
  import { drop } from "../lib/stores/drop.svelte";
  import { clips } from "../lib/stores/clips.svelte";
  import { focus } from "../lib/stores/focus.svelte";
  import { notes } from "../lib/stores/notes.svelte";
  import { vitals } from "../lib/stores/vitals.svelte";
  import {
    focusIsland,
    inTauri,
    nowPlaying,
    onMedia,
    onNativePointer,
    onSettingsChanged,
    onAppNotification,
    onDropReceived,
    onAudioLevel,
    onSuppressed,
    onToggle,
    setBeatActive,
    setHitRegion,
    type NowPlaying,
  } from "../lib/bridge";
  import { colorFromImage, toCss, type RGB } from "../lib/color";
  import {
    GEOMETRY,
    REST_HOT_ZONE,
    TUCKED_HOT_ZONE,
    TIMING,
    TUCKED,
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
  import { BeatFollower } from "../lib/beat";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { formatBytes } from "../lib/stores/vitals.svelte";
  import { formatClock, formatDuration, formatShortDate, meridiem } from "../lib/time";

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

  const clock = $derived(new Date(now));
  const path = $derived(islandPath(geometry, CENTER_X));
  const body = $derived(islandBounds({ ...geometry, shoulder: 0 }, CENTER_X));
  const glowOn = $derived(settings.glow && !!track?.playing && !suppressed);
  /** A full-screen app is in front and the island is resting: shrink to a slim tab. */
  const tucked = $derived(suppressed && mode === "rest");
  /** Rest or live content; the open panel has its own always-mounted layer. */
  const smallMode = $derived<"rest" | "live">(shownMode === "live" ? "live" : "rest");

  /** Where a mode's shape is heading, taking full-screen apps into account. */
  function targetOf(m: IslandMode): IslandGeometry {
    return m === "rest" && suppressed ? TUCKED : GEOMETRY[m];
  }
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
    if (progressToward(targetOf(mode), v) >= CONTENT_SWAP_AT) {
      shownMode = mode;
      contentVisible = true;
    }
  }

  function reportHitRegion(v: IslandGeometry) {
    // Cover both where the island is and where it is going, so input never lags the shape.
    let region: Rect = union(islandBounds(v, CENTER_X), islandBounds(targetOf(mode), CENTER_X));
    if (mode === "rest") {
      const zone = suppressed ? TUCKED_HOT_ZONE : REST_HOT_ZONE;
      region = inflate(region, zone.sides, zone.bottom);
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
    springs.to(targetOf(next), next === "rest" ? SPRINGS.close : SPRINGS.shape, reducedMotion.matches);
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
    const next = transition(mode, event);
    if (next === mode) return;
    const prev = mode;
    mode = next;
    if (next !== "live") cancel("live");
    if (next === "open" && prev !== "open") now = Date.now();
    animateTo(next);
  }

  function pointerEnter() {
    if (hovered) return;
    hovered = true;
    cancel("leave");
    const delay = tucked ? TIMING.tuckedIntent : TIMING.hoverIntent;
    if (mode !== "open") later("intent", delay, () => dispatch({ type: "hoverIntent" }));
  }

  function pointerLeave() {
    if (!hovered) return;
    hovered = false;
    cancel("intent");
    later("leave", TIMING.leaveGrace, () => dispatch({ type: "leave" }));
  }

  function toggle() {
    dispatch({ type: "toggle" });
    if (mode === "open") void focusIsland();
  }

  type LiveActivity =
    | { kind: "track" }
    | { kind: "timer"; title: string; detail: string }
    | { kind: "water" }
    | { kind: "copied"; text: string }
    | { kind: "notification"; app: string; title: string; body: string; icon: string | null }
    | { kind: "drop"; name: string; size: string };
  let live = $state<LiveActivity>({ kind: "track" });

  function showLiveActivity(activity: LiveActivity, duration = TIMING.liveDuration) {
    // Pop-ups also come out of the tucked tab; the island folds back afterwards.
    if (mode !== "rest" && mode !== "live") return;
    live = activity;
    dispatch({ type: "liveStart" });
    later("live", duration, () => dispatch({ type: "liveEnd", hovered }));
  }

  // ---------------------------------------------------------------- Notcher Drop

  /** Files are being dragged over the island. */
  let dragging = $state(false);

  // The QR code is ready whenever the Drop tab is open.
  $effect(() => {
    const showing = mode === "open" && tab === "drop";
    untrack(() => (showing ? drop.ensure() : drop.release()));
  });

  /** Dragging files onto the island opens the Drop tab; dropping shares them with the phone. */
  function onFileDrag(type: "enter" | "over" | "leave" | "drop", paths: string[] = []) {
    if (type === "enter") {
      dragging = true;
      tab = "drop";
      if (mode !== "open") dispatch({ type: "click" });
    } else if (type === "leave") {
      dragging = false;
    } else if (type === "drop") {
      dragging = false;
      void drop.share(paths);
    }
  }

  // ---------------------------------------------------------------- tools

  const TAB_KEY = "notcher.tab";
  let tab = $state<TabId>(readTab());
  function readTab(): TabId {
    try {
      const t = localStorage.getItem(TAB_KEY);
      if (TAB_IDS.includes(t as TabId)) return t as TabId;
    } catch {
      /* storage unavailable */
    }
    return "home";
  }
  $effect(() => {
    try {
      localStorage.setItem(TAB_KEY, tab);
    } catch {
      /* not persisted */
    }
  });
  // System stats stream only while the panel is open.
  $effect(() => {
    void vitals.setActive(mode === "open");
  });

  function onTick(t: number) {
    now = t;
    const { finished, water } = focus.update(t);
    if (finished) {
      const title =
        finished === "focus" ? "Focus session done" : finished === "countdown" ? "Time's up" : "Break's over";
      const detail = finished === "focus" ? `${focus.label} started` : finished === "countdown" ? "Your timer finished" : "Ready for the next session";
      showLiveActivity({ kind: "timer", title, detail }, 5000);
    } else if (water) {
      showLiveActivity({ kind: "water" }, 4000);
    }
  }

  // ---------------------------------------------------------------- beat glow

  let beatEl = $state<HTMLDivElement | null>(null);
  const beat = new BeatFollower();
  let beatFrame = 0;
  let beatLast = 0;
  const beatOn = $derived(
    settings.beatGlow && !!track?.playing && !tucked && mode !== "open" && !reducedMotion.matches,
  );
  $effect(() => {
    void setBeatActive(beatOn);
    if (!beatOn) beat.input(0);
    else startBeatLoop();
  });

  /** Writes straight to the element each frame; no Svelte state, so nothing else re-renders. */
  function startBeatLoop() {
    if (beatFrame) return;
    beatLast = performance.now();
    const frame = (t: number) => {
      const s = beat.step(t - beatLast, t);
      beatLast = t;
      if (beatEl) {
        beatEl.style.opacity = (s * 0.9).toFixed(3);
        beatEl.style.transform = `translateX(-50%) scaleX(${(0.55 + 0.45 * s).toFixed(3)})`;
      }
      beatFrame = !beatOn && beat.quiet ? 0 : requestAnimationFrame(frame);
    };
    beatFrame = requestAnimationFrame(frame);
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
    if (announce && changed && next?.playing && settings.trackChangeActivity) showLiveActivity({ kind: "track" });
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
    seedPreviewData();
    const t = params.get("tab");
    if (TAB_IDS.includes(t as TabId)) tab = t as TabId;
    if (t === "drop") drop.seedPreview();
    const m = params.get("mode");
    if (m === "open" || m === "live") {
      mode = m;
      shownMode = m;
      springs.to(GEOMETRY[m], undefined, true);
    }
    (window as unknown as { notcher: object }).notcher = {
      dispatch,
      notify: (title = "Mum", body = "Dinner at 8? Bring the cake 🎂", app = "WhatsApp") =>
        showLiveActivity({ kind: "notification", app, title, body, icon: null }, 4500),
      /** Feeds a fake beat: 120 bpm with a little noise. */
      beat: () =>
        setInterval(() => {
          const phase = (performance.now() % 500) / 500;
          beat.input(phase < 0.12 ? 0.85 : 0.3 + Math.random() * 0.1);
        }, 33),
      /** Pretends a full-screen app appeared or went away. */
      fullscreen: (value: boolean) => {
        suppressed = value;
        if (value && mode !== "rest") dispatch({ type: "suppress" });
        else animateTo(mode);
        reportHitRegion(geometry);
      },
      /** Pretends a file arrived from the phone. */
      received: (name = "IMG_2044.jpg") => showLiveActivity({ kind: "drop", name, size: "2.8 MB" }, 3800),
      /** Pretends files are being dragged over the island. */
      drag: (on = true) => onFileDrag(on ? "enter" : "leave"),
      nextTrack: () =>
        receiveTrack({ ...track!, title: "Paper Satellites", artist: "Low Orbit Club", positionMs: 0 }, true),
    };
  }

  /** Sample clips, notes and stats so every tab has something to show in the preview. */
  function seedPreviewData() {
    const t = Date.now();
    const clip = (id: number, text: string, ago: number, pinned = false) => ({
      id, kind: "text" as const, text, image: null, width: 0, height: 0, createdAt: t - ago, pinned,
    });
    clips.items = [
      clip(1, "git push -u origin feat/m2-tools", 120_000, true),
      clip(2, "https://example.com/design/islands-v2", 30_000),
      clip(3, "Meeting moved to 4:30, same room", 600_000),
      clip(4, "const spring = { stiffness: 380, damping: 32 };", 3_600_000),
    ];
    notes.items = [
      { id: 1, body: "Today\n[x] Ship clipboard tab\n[ ] Polish focus dial\n[ ] Water plants", pinned: true, updatedAt: t - 60_000 },
      { id: 2, body: "Ideas\nIsland that shows the next train", pinned: false, updatedAt: t - 7_200_000 },
    ];
    notes.select(notes.items[0]);
    const wave = (base: number, amp: number) =>
      Array.from({ length: 60 }, (_, i) => Math.max(0, Math.min(100, base + amp * Math.sin(i / 5) + (i % 7) * 1.5)));
    vitals.current = {
      cpu: 23, memUsed: 9.4 * 2 ** 30, memTotal: 16 * 2 ** 30, diskUsed: 312 * 2 ** 30, diskTotal: 512 * 2 ** 30,
      netRx: 1.8 * 2 ** 20, netTx: 220 * 2 ** 10, battery: { percent: 76, charging: false },
      cpuHistory: wave(22, 12), memHistory: wave(58, 3),
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
        if (value === suppressed) return;
        suppressed = value;
        if (value && mode !== "rest") dispatch({ type: "suppress" });
        else animateTo(mode); // tuck away or come back
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

    // Tick right after each new second so the clock's seconds change evenly.
    let tick: ReturnType<typeof setTimeout>;
    const scheduleTick = () => {
      const t = Date.now();
      onTick(t);
      tick = setTimeout(scheduleTick, 1000 - (t % 1000) + 2);
    };
    scheduleTick();
    unlisten.push(vitals.listen());
    unlisten.push(
      clips.listen((clip) => {
        if (settings.copyActivity) {
          const text = clip.kind === "image" ? `Image ${clip.width}×${clip.height}` : (clip.text ?? "").trim();
          showLiveActivity({ kind: "copied", text }, 1600);
        }
      }),
    );
    void clips.refresh();
    unlisten.push(
      onAudioLevel((level) => {
        if (beatOn) beat.input(level);
      }),
    );
    unlisten.push(drop.listen());
    void drop.refresh();
    unlisten.push(
      onDropReceived((f) => showLiveActivity({ kind: "drop", name: f.name, size: formatBytes(f.size) }, 3800)),
    );
    if (inTauri) {
      unlisten.push(
        getCurrentWebview().onDragDropEvent((e) => {
          const p = e.payload;
          onFileDrag(p.type, p.type === "drop" ? p.paths : []);
        }),
      );
    }
    unlisten.push(
      onAppNotification((n) => {
        if (settings.notifications) showLiveActivity({ kind: "notification", ...n }, 4500);
      }),
    );
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
      clearTimeout(tick);
      document.removeEventListener("pointermove", onMove);
      document.documentElement.removeEventListener("pointerleave", onOut);
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("blur", onBlur);
      springs.stop();
      cancelAnimationFrame(beatFrame);
      for (const t of timers.values()) clearTimeout(t);
      for (const u of unlisten) void u.then((f) => f());
    };
  });
</script>

<div class="stage" class:tucked style:--glow={glowCss}>
  <!-- Shadow and glow use box-shadow on a plain box: far cheaper to repaint every
       frame than SVG filters. -->
  <div
    class="halo"
    style:left="{body.x}px"
    style:width="{body.width}px"
    style:height="{body.height}px"
    style:border-radius="0 0 {geometry.radius * 1.15}px {geometry.radius * 1.15}px"
  >
    <div class="shadow"></div>
    <div class="glow" class:on={glowOn} class:breathe={glowOn && mode === "rest"}></div>
    <div class="beat" bind:this={beatEl}></div>
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
    <!-- The content is laid out at its final size and only clipped by the growing
         island, so the browser never re-lays it out during the animation. -->
    <div
      class="content mode-{smallMode}"
      class:visible={contentVisible && shownMode !== "open"}
      style:width="{GEOMETRY[smallMode].width}px"
      style:height="{GEOMETRY[smallMode].height}px"
      style:margin-left="{-GEOMETRY[smallMode].width / 2}px"
    >
      {#if smallMode === "rest"}
        <div class="rest-row">
          {#if track?.playing}<Equalizer playing size={12} />{/if}
          {#if focus.active}
            <span class="rest-timer" class:paused={!focus.running}>
              <Icon name="timer" size={13} />{formatDuration(focus.remaining)}
            </span>
          {:else}
            <span class="rest-date">{formatShortDate(clock)}</span>
          {/if}
          <span class="rest-time">
            {formatClock(clock, settings.clock24h, true)}{#if !settings.clock24h}<small>{meridiem(clock)}</small>{/if}
          </span>
        </div>
      {:else}
        <div class="live-row">
          {#if live.kind === "track" && track}
            <Art src={track.art} size={54} radius={14} />
            <div class="stack">
              <span class="eyebrow">Now playing</span>
              <strong>{track.title}</strong>
              <span class="muted">{track.artist}</span>
            </div>
            <Equalizer playing={track.playing} size={20} />
          {:else if live.kind === "timer"}
            <span class="badge"><Icon name="timer" size={24} /></span>
            <div class="stack">
              <span class="eyebrow">Focus</span>
              <strong>{live.title}</strong>
              <span class="muted">{live.detail}</span>
            </div>
          {:else if live.kind === "water"}
            <span class="badge"><Icon name="drop" size={24} /></span>
            <div class="stack">
              <span class="eyebrow">Hydrate</span>
              <strong>Time for some water</strong>
              <span class="muted">A glass now keeps you sharp</span>
            </div>
          {:else if live.kind === "notification"}
            <span class="badge">
              {#if live.icon}<img src={live.icon} alt="" />{:else}<Icon name="bell" size={24} />{/if}
            </span>
            <div class="stack">
              <span class="eyebrow">{live.app || "Notification"}</span>
              <strong>{live.title}</strong>
              {#if live.body}<span class="muted">{live.body}</span>{/if}
            </div>
          {:else if live.kind === "drop"}
            <span class="badge"><Icon name="beam" size={24} /></span>
            <div class="stack">
              <span class="eyebrow">From your phone</span>
              <strong>{live.name}</strong>
              <span class="muted">{live.size} · saved to Notcher Drop</span>
            </div>
          {:else if live.kind === "copied"}
            <span class="badge"><Icon name="copy" size={22} /></span>
            <div class="stack">
              <span class="eyebrow">Copied</span>
              <strong>{live.text}</strong>
            </div>
          {/if}
        </div>
      {/if}
    </div>
    <!-- The panel stays mounted (just hidden) so opening never has to build it
         mid-animation, which used to cost a dropped frame or two. -->
    <div
      class="content mode-open"
      class:visible={contentVisible && shownMode === "open"}
      style:width="{GEOMETRY.open.width}px"
      style:height="{GEOMETRY.open.height}px"
      style:margin-left="{-GEOMETRY.open.width / 2}px"
      inert={shownMode !== "open"}
    >
      <OpenPanel {track} {clock} clock24h={settings.clock24h} {dragging} bind:tab />
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
  .stage.tucked .content {
    opacity: 0;
    transition: opacity 120ms ease;
  }
  .stage.tucked .shadow {
    opacity: 0.4;
  }
  .shape {
    position: absolute;
    inset: 0;
    overflow: visible;
    /* Own compositor layer: redrawing the outline never repaints the content. */
    will-change: transform;
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
    will-change: transform;
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
  /* Thin light under the bottom edge that moves with the music (driven from script). */
  .halo > .beat {
    inset: auto;
    left: 50%;
    bottom: -7px;
    width: 72%;
    height: 12px;
    border-radius: 50%;
    opacity: 0;
    transform: translateX(-50%) scaleX(0.55);
    background: radial-gradient(closest-side, var(--glow, var(--accent)), transparent);
    will-change: opacity, transform;
    pointer-events: none;
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
    top: 0;
    left: 50%;
    contain: layout style paint;
    opacity: 0;
    visibility: hidden;
    transform: scale(0.96) translateY(-4px);
    transform-origin: 50% 0;
    transition:
      opacity 90ms ease,
      transform 90ms ease,
      visibility 0s linear 90ms;
    will-change: opacity, transform;
  }
  .content.visible {
    opacity: 1;
    visibility: visible;
    transform: none;
    transition:
      opacity 200ms ease,
      transform 320ms cubic-bezier(0.2, 0.9, 0.25, 1.1),
      visibility 0s;
  }

  .rest-row {
    height: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 14px;
    padding: 0 16px;
    font-size: 13px;
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }
  /* Date and time share one quiet style; tabular digits keep the seconds from jiggling. */
  .rest-date,
  .rest-time,
  .rest-time small {
    font-size: 13px;
    font-weight: 400;
    color: var(--muted);
  }
  .rest-time small {
    margin-left: 4px;
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

  .badge img {
    width: 34px;
    height: 34px;
    object-fit: contain;
  }
  .badge {
    width: 54px;
    height: 54px;
    flex: none;
    display: grid;
    place-items: center;
    border-radius: 14px;
    background: var(--surface-2);
    color: var(--glow, var(--accent));
  }
  .rest-timer {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    font-weight: 600;
    color: var(--glow, var(--accent));
  }
  .rest-timer.paused { color: var(--muted); }

  @media (prefers-reduced-motion: reduce) {
    .content, .content.visible { transform: none; }
    .glow.breathe { animation: none; opacity: 0.6; }
  }
</style>
