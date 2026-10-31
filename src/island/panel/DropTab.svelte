<script lang="ts">
  import { open as pickFiles } from "@tauri-apps/plugin-dialog";
  import { onMount } from "svelte";
  import Icon, { type IconName } from "../../components/Icon.svelte";
  import { inTauri } from "../../lib/bridge";
  import { drop } from "../../lib/stores/drop.svelte";
  import { formatBytes } from "../../lib/stores/vitals.svelte";

  let { dragging = false }: { dragging?: boolean } = $props();

  const s = $derived(drop.state);

  // A clock for "connected" and "2 min ago"; the phone checks in every 2.5 s.
  let now = $state(Date.now());
  onMount(() => {
    const t = setInterval(() => (now = Date.now()), 1000);
    return () => clearInterval(t);
  });
  const connected = $derived(!!s.device && now - s.deviceSeenAt < 8_000);

  type Item = {
    key: string;
    id: number;
    dir: "in" | "out";
    name: string;
    size: number;
    at: number;
    progress: number | null;
    meta: string;
    failed: boolean;
    done: boolean;
  };

  const items = $derived.by<Item[]>(() => {
    const phone = s.device ?? "phone";
    const incoming = s.received.map<Item>((r) => {
      const receiving = r.status === "receiving";
      const meta = receiving
        ? `${formatBytes(r.done)} of ${formatBytes(r.size)}${r.rate ? ` · ${formatBytes(r.rate)}/s` : ""}`
        : r.status === "failed"
          ? "Didn't finish"
          : `From ${phone} · ${formatBytes(r.size)} · ${ago(r.at)}`;
      return {
        key: `in-${r.id}`,
        id: r.id,
        dir: "in",
        name: r.name,
        size: r.size,
        at: r.at,
        progress: receiving ? (r.size ? r.done / r.size : 0) : null,
        meta,
        failed: r.status === "failed",
        done: r.status === "done",
      };
    });
    const outgoing = s.shared.map<Item>((f) => ({
      key: `out-${f.id}`,
      id: f.id,
      dir: "out",
      name: f.name,
      size: f.size,
      at: f.at,
      progress: null,
      meta: `To ${phone} · ${formatBytes(f.size)} · ${f.downloads ? (f.downloads === 1 ? "Saved" : `Saved ${f.downloads}×`) : "Ready"}`,
      failed: false,
      done: f.downloads > 0,
    }));
    return [...incoming, ...outgoing].sort((a, b) => b.at - a.at);
  });

  $effect(() => {
    drop.loadThumbs(items.filter((it) => it.done || it.dir === "out"));
  });

  function ago(t: number) {
    const sec = Math.max(0, (now - t) / 1000);
    if (sec < 45) return "just now";
    if (sec < 3600) return `${Math.round(sec / 60)} min ago`;
    return `${Math.round(sec / 3600)} h ago`;
  }

  const KINDS: { re: RegExp; icon: IconName; hue: string }[] = [
    { re: /\.(png|jpe?g|webp|gif|bmp|heic|heif|svg|raw|dng)$/i, icon: "image", hue: "#f472b6" },
    { re: /\.(mp4|mov|mkv|avi|webm|m4v|3gp)$/i, icon: "film", hue: "#a78bfa" },
    { re: /\.(mp3|m4a|wav|flac|aac|ogg|opus)$/i, icon: "note", hue: "#fb923c" },
    { re: /\.(zip|rar|7z|tar|gz|apk)$/i, icon: "box", hue: "#facc15" },
    { re: /\.(pdf|docx?|pptx?|xlsx?|txt|md|csv|rtf|odt)$/i, icon: "doc", hue: "#60a5fa" },
  ];
  const kind = (name: string) => KINDS.find((k) => k.re.test(name)) ?? { icon: "doc" as IconName, hue: "#94a3b8" };

  async function browse() {
    if (!inTauri) return drop.share(["Holiday photos.zip"]);
    const picked = await pickFiles({ multiple: true, directory: false, title: "Send to your phone" }).catch(() => null);
    if (!picked) return;
    await drop.share(Array.isArray(picked) ? picked : [picked]);
  }

  // Progress ring geometry.
  const R = 17;
  const C = 2 * Math.PI * R;
</script>

<div class="drop" class:dragging>
  {#if !s.active}
    <div class="paused">
      <span class="orb" aria-hidden="true"><Icon name="beam" size={26} /></span>
      <div>
        <h3>Notcher Drop is paused</h3>
        {#if s.error}
          <p class="error">{s.error}</p>
        {:else}
          <p>Send files between your phone and this PC over Wi-Fi.</p>
        {/if}
      </div>
      <button class="primary" disabled={drop.starting} onclick={() => drop.start()}>
        {drop.starting ? "Starting…" : s.error ? "Try again" : "Show QR code"}
      </button>
    </div>
  {:else}
    <div class="live">
      <aside class="pair" class:connected>
        <div class="ring">
          <div class="qr">{#if s.qr}<img src={s.qr} alt="QR code for Notcher Drop" />{/if}</div>
        </div>
        <span class="status">
          <i></i>{connected ? `${s.device} connected` : "Waiting for your phone"}
        </span>
        <span class="hint">{connected ? "Send from your phone" : "Scan with your camera"}</span>
        <div class="tools">
          <button title="Open the Notcher Drop folder" onclick={() => drop.open(null)}><Icon name="folder" size={14} />Folder</button>
          <button title="Stop sharing" onclick={() => drop.stop()}><Icon name="stop" size={12} />Stop</button>
        </div>
      </aside>

      <section class="feed">
        <button class="zone" onclick={browse}>
          <span class="zone-icon"><Icon name="upload" size={16} /></span>
          <span><strong>Drop files here</strong> or browse to send them to your phone</span>
        </button>

        <div class="list">
          {#each items as it (it.key)}
            {@const k = kind(it.name)}
            {@const thumb = drop.thumbs[it.id] ?? null}
            <div class="item" class:failed={it.failed} style:--hue={k.hue}>
              <span class="tile">
                {#if thumb}
                  <img src={thumb} alt="" />
                {:else}
                  <Icon name={k.icon} size={17} />
                {/if}
                {#if it.progress !== null}
                  <svg class="ring-progress" viewBox="0 0 40 40" aria-hidden="true">
                    <circle cx="20" cy="20" r={R} />
                    <circle class="bar" cx="20" cy="20" r={R} stroke-dasharray={C} stroke-dashoffset={C * (1 - it.progress)} />
                  </svg>
                {/if}
              </span>
              <div class="meta">
                <span class="name">{it.name}</span>
                <span class="sub">{it.meta}</span>
              </div>
              <span class="dir {it.dir}" title={it.dir === "in" ? "From your phone" : "To your phone"}>
                <Icon name={it.dir === "in" ? "arrowDown" : "arrowUp"} size={12} />
              </span>
              <span class="actions">
                {#if it.dir === "in" && it.done}
                  <button title="Open" aria-label="Open {it.name}" onclick={() => drop.open(it.id)}><Icon name="external" size={14} /></button>
                  <button title="Show in folder" aria-label="Show {it.name} in folder" onclick={() => drop.open(it.id, true)}><Icon name="folder" size={14} /></button>
                {:else if it.dir === "out"}
                  <button title="Stop sharing" aria-label="Stop sharing {it.name}" onclick={() => drop.unshare(it.id)}><Icon name="close" size={13} /></button>
                {/if}
              </span>
            </div>
          {:else}
            <p class="empty">Nothing here yet. Files you send or receive show up here.</p>
          {/each}
        </div>
      </section>
    </div>
  {/if}
  <div class="catch" aria-hidden="true">
    <span class="catch-icon"><Icon name="upload" size={26} /></span>
    <strong>Drop to send to your phone</strong>
    <span>{connected ? `${s.device} can save them right away` : "Scan the code with your phone to get them"}</span>
  </div>
</div>

<style>
  .drop { position: relative; height: 100%; }
  h3 { margin: 0; font-size: 15px; }
  p { margin: 2px 0 0; color: var(--muted); font-size: 12.5px; }
  button { font: inherit; cursor: pointer; }
  .error { color: #ff8a8a; }

  /* ---- paused */
  .paused { height: 100%; display: flex; align-items: center; justify-content: center; gap: 16px; }
  .orb {
    width: 54px; height: 54px; border-radius: 18px; display: grid; place-items: center;
    color: var(--glow, var(--accent)); background: var(--surface-1); box-shadow: inset 0 0 0 1px var(--hairline);
  }
  .primary {
    height: 34px; padding: 0 16px; border: 0; border-radius: 11px; margin-left: 8px;
    background: var(--glow, var(--accent)); color: #fff; font-weight: 600; font-size: 13px;
    transition: transform 140ms, filter 140ms;
  }
  .primary:hover { filter: brightness(1.1); }
  .primary:active { transform: scale(0.96); }
  .primary:disabled { opacity: 0.6; cursor: default; }

  /* ---- live */
  .live { height: 100%; display: grid; grid-template-columns: 180px 1fr; gap: 14px; }

  .pair {
    display: flex; flex-direction: column; align-items: center; gap: 6px; padding: 12px 10px 10px;
    border-radius: 22px; text-align: center;
    background:
      radial-gradient(120% 70% at 50% 0%, color-mix(in srgb, var(--glow, var(--accent)) 22%, transparent), transparent 70%),
      var(--surface-1);
    box-shadow: inset 0 0 0 1px var(--hairline);
  }
  .ring {
    position: relative; padding: 3px; border-radius: 19px; overflow: hidden; isolation: isolate;
    animation: qr-in 420ms cubic-bezier(0.2, 0.9, 0.25, 1.15);
  }
  .ring::before {
    content: ""; position: absolute; inset: -40%; z-index: -1;
    background: conic-gradient(from 0deg, transparent 0 55%, var(--glow, var(--accent)) 75%, #fff 80%, transparent 85%);
    animation: spin 3.2s linear infinite;
  }
  .connected .ring::before { animation-duration: 9s; opacity: 0.55; }
  .qr { width: 118px; height: 118px; padding: 8px; border-radius: 16px; background: #fff; }
  .qr img { width: 100%; height: 100%; display: block; image-rendering: pixelated; }
  @keyframes spin { to { transform: rotate(1turn); } }
  @keyframes qr-in { from { opacity: 0; transform: scale(0.88); } }

  .status { display: inline-flex; align-items: center; gap: 6px; margin-top: 4px; font-size: 12.5px; font-weight: 600; }
  .status i { width: 7px; height: 7px; border-radius: 50%; background: var(--glow, var(--accent)); animation: beacon 1.6s ease-out infinite; }
  .connected .status i { background: #34d399; animation: none; box-shadow: 0 0 8px #34d399; }
  @keyframes beacon { 0% { box-shadow: 0 0 0 0 color-mix(in srgb, var(--glow, var(--accent)) 70%, transparent); } 100% { box-shadow: 0 0 0 7px transparent; } }
  .hint { font-size: 11px; color: var(--muted); }
  .tools { display: flex; gap: 4px; margin-top: auto; }
  .tools button {
    display: inline-flex; align-items: center; gap: 4px; height: 24px; padding: 0 9px; border: 0; border-radius: 8px;
    background: var(--surface-2); color: var(--muted); font-size: 11.5px; transition: color 140ms, background 140ms;
  }
  .tools button:hover { color: var(--fg); }

  .feed { min-width: 0; min-height: 0; display: flex; flex-direction: column; gap: 8px; }
  .zone {
    flex: none; display: flex; align-items: center; gap: 10px; height: 44px; padding: 0 12px; border-radius: 14px;
    border: 1.5px dashed color-mix(in srgb, var(--glow, var(--accent)) 45%, var(--hairline));
    background: color-mix(in srgb, var(--glow, var(--accent)) 7%, transparent);
    color: var(--muted); font-size: 12.5px; text-align: left;
    transition: background 160ms, border-color 160ms, transform 160ms;
  }
  .zone strong { color: var(--fg); font-weight: 600; }
  .zone:hover { background: color-mix(in srgb, var(--glow, var(--accent)) 14%, transparent); }
  .zone:active { transform: scale(0.99); }
  .zone-icon { width: 26px; height: 26px; border-radius: 8px; display: grid; place-items: center; background: var(--glow, var(--accent)); color: #fff; }

  .list { flex: 1; min-height: 0; overflow-y: auto; display: flex; flex-direction: column; gap: 4px; scrollbar-width: thin; padding-right: 2px; }
  .item {
    position: relative; display: flex; align-items: center; gap: 10px; padding: 6px 8px; border-radius: 13px;
    transition: background 140ms;
    animation: item-in 280ms cubic-bezier(0.2, 0.9, 0.25, 1);
  }
  .item:hover { background: var(--surface-1); }
  @keyframes item-in { from { opacity: 0; transform: translateY(-6px) scale(0.98); } }
  .tile {
    position: relative; flex: none; width: 38px; height: 38px; border-radius: 11px; display: grid; place-items: center;
    color: var(--hue); background: color-mix(in srgb, var(--hue) 16%, transparent); overflow: hidden;
  }
  .tile img { width: 100%; height: 100%; object-fit: cover; }
  .ring-progress { position: absolute; inset: 0; width: 100%; height: 100%; transform: rotate(-90deg); }
  .ring-progress circle { fill: none; stroke: color-mix(in srgb, var(--hue) 25%, transparent); stroke-width: 3; }
  .ring-progress .bar { stroke: var(--hue); stroke-linecap: round; transition: stroke-dashoffset 160ms linear; }
  .meta { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 1px; }
  .name { font-size: 12.5px; font-weight: 500; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .sub { font-size: 11px; color: var(--muted); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; font-variant-numeric: tabular-nums; }
  .failed .sub { color: #ff8a8a; }
  .dir { flex: none; width: 20px; height: 20px; border-radius: 50%; display: grid; place-items: center; transition: opacity 120ms; }
  .dir.in { color: #34d399; background: rgb(52 211 153 / 0.14); }
  .dir.out { color: var(--glow, var(--accent)); background: color-mix(in srgb, var(--glow, var(--accent)) 16%, transparent); }
  .actions { position: absolute; right: 6px; display: flex; gap: 2px; opacity: 0; transition: opacity 140ms; }
  .item:hover .actions { opacity: 1; }
  .item:hover .dir:has(+ .actions:not(:empty)) { opacity: 0; }
  .actions button {
    width: 28px; height: 28px; border: 0; border-radius: 9px; display: grid; place-items: center;
    background: var(--surface-2); color: var(--fg);
  }
  .actions button:hover { background: var(--glow, var(--accent)); color: #fff; }
  .empty { margin: 6px 4px; font-size: 12px; }

  /* ---- files dragged over the island */
  .catch {
    position: absolute; inset: -4px; border-radius: 22px; display: flex; flex-direction: column;
    align-items: center; justify-content: center; gap: 6px; pointer-events: none; text-align: center;
    background: color-mix(in srgb, var(--island-bg) 90%, transparent);
    backdrop-filter: blur(6px);
    border: 1.5px dashed var(--glow, var(--accent));
    opacity: 0; transform: scale(0.97); transition: opacity 160ms, transform 240ms cubic-bezier(0.2, 0.9, 0.25, 1.25);
  }
  .catch-icon {
    width: 52px; height: 52px; border-radius: 17px; display: grid; place-items: center; color: #fff;
    background: var(--glow, var(--accent)); box-shadow: 0 10px 30px -8px var(--glow, var(--accent));
    animation: bob 1.4s ease-in-out infinite;
  }
  @keyframes bob { 50% { transform: translateY(-4px); } }
  .catch strong { font-size: 14.5px; margin-top: 4px; }
  .catch span:last-child { font-size: 12px; color: var(--muted); }
  .dragging .catch { opacity: 1; transform: none; }
  @media (prefers-reduced-motion: reduce) {
    .ring::before, .ring, .item, .catch-icon, .status i { animation: none; }
  }
</style>
