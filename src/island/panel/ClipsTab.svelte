<script lang="ts">
  import { onMount } from "svelte";
  import Icon from "../../components/Icon.svelte";
  import { focusIsland, type Clip } from "../../lib/bridge";
  import { clips } from "../../lib/stores/clips.svelte";

  onMount(() => void clips.refresh());

  let searchTimer: ReturnType<typeof setTimeout> | undefined;
  function search(value: string) {
    clips.query = value;
    clearTimeout(searchTimer);
    searchTimer = setTimeout(() => void clips.refresh(), 150);
  }

  function ago(ms: number): string {
    const s = Math.max(0, Math.round((Date.now() - ms) / 1000));
    if (s < 60) return "now";
    if (s < 3600) return `${Math.floor(s / 60)}m`;
    if (s < 86400) return `${Math.floor(s / 3600)}h`;
    return `${Math.floor(s / 86400)}d`;
  }

  function act(e: Event, fn: () => void) {
    e.stopPropagation();
    fn();
  }
</script>

<div class="clips">
  <div class="bar">
    <label class="search">
      <Icon name="search" size={14} />
      <input
        placeholder="Search clipboard"
        value={clips.query}
        oninput={(e) => search(e.currentTarget.value)}
        onfocus={() => void focusIsland()}
        spellcheck="false"
      />
    </label>
    {#if clips.items.some((c) => !c.pinned)}
      <button class="ghost" onclick={() => void clips.clear()}>Clear</button>
    {/if}
  </div>

  {#if clips.items.length === 0}
    <p class="empty">
      {clips.query ? "No matches." : "Copy something and it will appear here. Passwords from password managers are skipped."}
    </p>
  {:else}
    <ul>
      {#each clips.items as clip (clip.id)}
        {@render row(clip)}
      {/each}
    </ul>
  {/if}
</div>

{#snippet row(clip: Clip)}
  <li>
    <button class="item" class:copied={clips.justCopied === clip.id} onclick={() => void clips.copy(clip)} title="Copy again">
      {#if clip.kind === "image" && clip.image}
        <img src={clip.image} alt="" />
        <span class="muted">{clip.width}×{clip.height}</span>
      {:else}
        <span class="text">{clip.text}</span>
      {/if}
      <span class="meta">
        {#if clips.justCopied === clip.id}
          <span class="ok"><Icon name="check" size={14} /> Copied</span>
        {:else}
          <span class="muted">{ago(clip.createdAt)}</span>
        {/if}
      </span>
    </button>
    <div class="actions">
      <button class:on={clip.pinned} aria-label={clip.pinned ? "Unpin" : "Pin"} onclick={(e) => act(e, () => void clips.togglePin(clip))}>
        <Icon name="pin" size={14} />
      </button>
      <button aria-label="Delete" onclick={(e) => act(e, () => void clips.remove(clip))}>
        <Icon name="trash" size={14} />
      </button>
    </div>
  </li>
{/snippet}

<style>
  .clips { height: 100%; display: flex; flex-direction: column; gap: 8px; min-height: 0; }
  .bar { display: flex; gap: 8px; align-items: center; }
  .search {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 10px;
    height: 32px;
    border-radius: 10px;
    background: var(--surface-1);
    box-shadow: inset 0 0 0 1px var(--hairline);
    color: var(--muted);
  }
  .search:focus-within { box-shadow: inset 0 0 0 1.5px var(--glow, var(--accent)); }
  input {
    flex: 1;
    border: 0;
    outline: 0;
    background: transparent;
    color: var(--fg);
    font: inherit;
    font-size: 13px;
    user-select: text;
    -webkit-user-select: text;
  }
  .ghost {
    border: 0;
    background: transparent;
    color: var(--muted);
    font: inherit;
    font-size: 12.5px;
    cursor: pointer;
    padding: 6px 8px;
    border-radius: 8px;
  }
  .ghost:hover { background: var(--surface-2); color: var(--fg); }
  ul {
    list-style: none;
    margin: 0;
    padding: 0 4px 0 0;
    overflow-y: auto;
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
    scrollbar-width: thin;
  }
  li { position: relative; }
  .item {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 76px 8px 12px;
    border: 0;
    border-radius: 12px;
    background: var(--surface-1);
    color: var(--fg);
    font: inherit;
    text-align: left;
    cursor: pointer;
    transition: background 150ms, transform 180ms cubic-bezier(0.2, 0.8, 0.2, 1);
  }
  .item:hover { background: var(--surface-2); }
  .item:active { transform: scale(0.985); }
  .item.copied { box-shadow: inset 0 0 0 1.5px var(--glow, var(--accent)); }
  .text {
    flex: 1;
    min-width: 0;
    font-size: 13px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  img { height: 40px; max-width: 120px; border-radius: 6px; object-fit: cover; }
  .meta { margin-left: auto; flex: none; }
  .muted { color: var(--muted); font-size: 11.5px; }
  .ok { display: inline-flex; gap: 4px; align-items: center; color: var(--glow, var(--accent)); font-size: 12px; }
  .actions {
    position: absolute;
    right: 8px;
    top: 50%;
    transform: translateY(-50%);
    display: flex;
    gap: 2px;
    opacity: 0;
    transition: opacity 150ms;
  }
  li:hover .actions, .actions:focus-within { opacity: 1; }
  .actions button {
    width: 28px;
    height: 28px;
    display: grid;
    place-items: center;
    border: 0;
    border-radius: 8px;
    background: var(--surface-2);
    color: var(--muted);
    cursor: pointer;
  }
  .actions button:hover { color: var(--fg); }
  .actions button.on { color: var(--glow, var(--accent)); opacity: 1; }
  li:has(.on) .actions { opacity: 1; }
  .empty { color: var(--muted); font-size: 13px; margin: auto; text-align: center; max-width: 340px; }
</style>
