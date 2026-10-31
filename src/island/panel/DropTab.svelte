<script lang="ts">
  import { open as pickFiles } from "@tauri-apps/plugin-dialog";
  import Icon from "../../components/Icon.svelte";
  import { inTauri } from "../../lib/bridge";
  import { drop } from "../../lib/stores/drop.svelte";
  import { formatBytes } from "../../lib/stores/vitals.svelte";

  let { dragging = false }: { dragging?: boolean } = $props();

  const s = $derived(drop.state);
  const host = $derived(s.url ? s.url.replace(/^http:\/\//, "").replace(/\/\?.*$/, "") : "");

  async function addFiles() {
    if (!inTauri) return drop.share(["Holiday photos.zip"]);
    const picked = await pickFiles({ multiple: true, directory: false, title: "Share with your phone" }).catch(() => null);
    if (!picked) return;
    await drop.share(Array.isArray(picked) ? picked : [picked]);
  }

  function ext(name: string) {
    const m = /\.([a-z0-9]{1,4})$/i.exec(name);
    return m ? m[1] : "file";
  }
</script>

<div class="drop" class:dragging>
  {#if !s.active}
    <div class="intro">
      <div class="visual" aria-hidden="true">
        <span class="dev phone"><Icon name="phone" size={30} /></span>
        <span class="wave"><i></i><i></i><i></i></span>
        <span class="dev pc"><Icon name="beam" size={30} /></span>
      </div>
      <div class="copy">
        <h3>Notcher Drop</h3>
        <p>Send photos and files between your phone and this PC. No app or cable, just the same Wi-Fi.</p>
        {#if s.error}<p class="error">{s.error}</p>{/if}
        <div class="row">
          <button class="primary" disabled={drop.starting} onclick={() => drop.start()}>
            {drop.starting ? "Starting…" : "Show QR code"}
          </button>
          <button class="ghost" onclick={addFiles}><Icon name="plus" size={14} />Share files</button>
        </div>
        <span class="hint">Tip: drag files onto the island to share them.</span>
      </div>
    </div>
  {:else}
    <div class="live">
      <div class="code">
        <div class="qr">
          {#if s.qr}<img src={s.qr} alt="QR code for Notcher Drop" />{/if}
        </div>
        <strong>Scan with your phone</strong>
        <span class="addr" title="Phone and PC must be on the same Wi-Fi. If Windows asks, allow Notcher on private networks.">Same Wi-Fi · {host}</span>
        <button class="link" onclick={() => drop.stop()}>Stop sharing</button>
      </div>
      <div class="lists">
        <section>
          <header>
            <span>From your phone</span>
            <span class="actions">
              {#if s.received.some((r) => r.status !== "receiving")}
                <button class="mini" onclick={() => drop.clearReceived()}>Clear</button>
              {/if}
              <button class="mini" title={s.folder} onclick={() => drop.open(null)}><Icon name="folder" size={13} />Folder</button>
            </span>
          </header>
          {#each s.received as r (r.id)}
            <div class="item {r.status}">
              <span class="badge">{ext(r.name)}</span>
              <div class="meta">
                <span class="name">{r.name}</span>
                {#if r.status === "receiving"}
                  <div class="bar"><div style:width="{r.size ? (r.done / r.size) * 100 : 0}%"></div></div>
                {:else if r.status === "done"}
                  <span class="info">{formatBytes(r.size)}</span>
                {:else}
                  <span class="info">Didn't finish</span>
                {/if}
              </div>
              {#if r.status === "done"}
                <button class="icon" title="Open" aria-label="Open {r.name}" onclick={() => drop.open(r.id)}><Icon name="external" size={14} /></button>
                <button class="icon" title="Show in folder" aria-label="Show {r.name} in folder" onclick={() => drop.open(r.id, true)}><Icon name="folder" size={14} /></button>
              {:else if r.status === "receiving"}
                <span class="pct">{r.size ? Math.round((r.done / r.size) * 100) : 0}%</span>
              {/if}
            </div>
          {:else}
            <p class="empty">Files you send from your phone are saved to Downloads › Notcher Drop.</p>
          {/each}
        </section>
        <section>
          <header>
            <span>To your phone</span>
            <span class="actions"><button class="mini" onclick={addFiles}><Icon name="plus" size={13} />Add</button></span>
          </header>
          {#each s.shared as f (f.id)}
            <div class="item">
              <span class="badge">{ext(f.name)}</span>
              <div class="meta">
                <span class="name">{f.name}</span>
                <span class="info">{formatBytes(f.size)}</span>
              </div>
              <button class="icon" title="Stop sharing" aria-label="Stop sharing {f.name}" onclick={() => drop.unshare(f.id)}><Icon name="close" size={13} /></button>
            </div>
          {:else}
            <p class="empty">Drag files onto the island or add them here, then tap Save on your phone.</p>
          {/each}
        </section>
      </div>
    </div>
  {/if}
  <div class="catch" aria-hidden="true">
    <Icon name="upload" size={30} />
    <strong>Drop to share with your phone</strong>
  </div>
</div>

<style>
  .drop { position: relative; height: 100%; }
  h3 { margin: 0; font-size: 18px; }
  p { margin: 0; color: var(--muted); font-size: 13px; line-height: 1.45; }
  button { font: inherit; cursor: pointer; }

  /* ---- before a session */
  .intro { height: 100%; display: grid; grid-template-columns: 220px 1fr; align-items: center; gap: 24px; padding: 0 6px; }
  .visual {
    height: 170px; border-radius: 22px; background: var(--surface-1);
    display: flex; align-items: center; justify-content: center; gap: 10px;
    box-shadow: inset 0 0 0 1px var(--hairline);
  }
  .dev { width: 58px; height: 58px; border-radius: 18px; display: grid; place-items: center; background: var(--surface-2); color: var(--fg); }
  .pc { color: var(--glow, var(--accent)); }
  .wave { display: flex; gap: 5px; }
  .wave i { width: 6px; height: 6px; border-radius: 50%; background: var(--glow, var(--accent)); animation: hop 1.4s ease-in-out infinite; }
  .wave i:nth-child(2) { animation-delay: 0.15s; }
  .wave i:nth-child(3) { animation-delay: 0.3s; }
  @keyframes hop { 0%, 60%, 100% { opacity: 0.25; transform: translateX(0); } 30% { opacity: 1; transform: translateX(3px); } }
  .copy { display: flex; flex-direction: column; gap: 10px; align-items: flex-start; }
  .row { display: flex; gap: 8px; margin-top: 4px; }
  .primary {
    height: 34px; padding: 0 16px; border: 0; border-radius: 11px;
    background: var(--glow, var(--accent)); color: #fff; font-weight: 600; font-size: 13px;
    transition: transform 140ms, filter 140ms;
  }
  .primary:hover { filter: brightness(1.1); }
  .primary:active { transform: scale(0.96); }
  .primary:disabled { opacity: 0.6; cursor: default; }
  .ghost {
    display: inline-flex; align-items: center; gap: 6px; height: 34px; padding: 0 14px;
    border: 0; border-radius: 11px; background: var(--surface-2); color: var(--fg); font-size: 13px;
  }
  .ghost:hover { background: var(--surface-1); box-shadow: inset 0 0 0 1px var(--hairline); }
  .hint { font-size: 11.5px; color: var(--muted); }
  .error { color: #ff8a8a; }

  /* ---- during a session */
  .live { height: 100%; display: grid; grid-template-columns: 172px 1fr; gap: 14px; }
  .code { display: flex; flex-direction: column; align-items: center; gap: 4px; text-align: center; }
  .qr {
    width: 150px; height: 150px; padding: 9px; border-radius: 18px; background: #fff;
    box-shadow: 0 8px 26px -10px var(--glow, var(--accent));
    animation: qr-in 380ms cubic-bezier(0.2, 0.9, 0.25, 1.15);
    margin-bottom: 6px;
  }
  .qr img { width: 100%; height: 100%; display: block; image-rendering: pixelated; }
  @keyframes qr-in { from { opacity: 0; transform: scale(0.86); } }
  .code strong { font-size: 13px; }
  .addr { font-size: 11px; color: var(--muted); max-width: 100%; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .link { border: 0; background: none; color: var(--muted); font-size: 12px; text-decoration: underline; text-underline-offset: 2px; padding: 4px; }
  .link:hover { color: var(--fg); }

  .lists { min-height: 0; overflow-y: auto; display: flex; flex-direction: column; gap: 10px; padding-right: 2px; scrollbar-width: thin; }
  section { display: flex; flex-direction: column; gap: 6px; }
  section header { display: flex; align-items: center; justify-content: space-between; font-size: 11.5px; text-transform: uppercase; letter-spacing: 0.07em; color: var(--muted); }
  .actions { display: flex; gap: 4px; }
  .mini {
    display: inline-flex; align-items: center; gap: 4px; height: 22px; padding: 0 8px; border: 0; border-radius: 7px;
    background: var(--surface-1); color: var(--muted); font-size: 11.5px; text-transform: none; letter-spacing: 0;
  }
  .mini:hover { color: var(--fg); background: var(--surface-2); }
  .item {
    display: flex; align-items: center; gap: 10px; padding: 7px 8px; border-radius: 12px; background: var(--surface-1);
    animation: item-in 260ms cubic-bezier(0.2, 0.9, 0.25, 1);
  }
  @keyframes item-in { from { opacity: 0; transform: translateY(-4px); } }
  .badge {
    flex: none; width: 30px; height: 30px; border-radius: 9px; display: grid; place-items: center;
    background: var(--surface-2); color: var(--glow, var(--accent)); font-size: 9.5px; font-weight: 700; text-transform: uppercase;
  }
  .meta { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 3px; }
  .name { font-size: 12.5px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .info { font-size: 11px; color: var(--muted); }
  .failed .info { color: #ff8a8a; }
  .bar { height: 3px; border-radius: 3px; background: var(--surface-2); overflow: hidden; }
  .bar > div { height: 100%; background: var(--glow, var(--accent)); transition: width 150ms linear; }
  .pct { font-size: 11px; color: var(--muted); font-variant-numeric: tabular-nums; width: 32px; text-align: right; }
  .icon {
    flex: none; width: 26px; height: 26px; border: 0; border-radius: 8px; display: grid; place-items: center;
    background: transparent; color: var(--muted);
  }
  .icon:hover { background: var(--surface-2); color: var(--fg); }
  .empty { font-size: 12px; padding: 8px 10px; border-radius: 12px; border: 1px dashed var(--hairline); }

  /* ---- files dragged over the island */
  .catch {
    position: absolute; inset: 0; border-radius: 20px; display: flex; flex-direction: column;
    align-items: center; justify-content: center; gap: 10px; pointer-events: none;
    background: color-mix(in srgb, var(--island-bg) 88%, transparent);
    border: 1.5px dashed var(--glow, var(--accent)); color: var(--glow, var(--accent));
    opacity: 0; transform: scale(0.98); transition: opacity 160ms, transform 220ms cubic-bezier(0.2, 0.9, 0.25, 1.2);
  }
  .catch strong { color: var(--fg); font-size: 14px; }
  .dragging .catch { opacity: 1; transform: none; }
  @media (prefers-reduced-motion: reduce) {
    .wave i, .qr, .item { animation: none; }
  }
</style>
