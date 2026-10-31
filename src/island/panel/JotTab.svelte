<script lang="ts">
  import { onMount } from "svelte";
  import Icon from "../../components/Icon.svelte";
  import { focusIsland } from "../../lib/bridge";
  import { notes, noteTitle } from "../../lib/stores/notes.svelte";

  onMount(() => {
    void notes.load();
    return () => void notes.flush();
  });
</script>

<div class="jot">
  <aside>
    <button class="new" onclick={() => void notes.create()}><Icon name="plus" size={14} /> New note</button>
    <ul>
      {#each notes.items as note (note.id)}
        <li>
          <button class="title" class:active={note.id === notes.selectedId} onclick={() => notes.select(note)}>
            {#if note.pinned}<Icon name="pin" size={12} />{/if}
            <span>{noteTitle(note.id === notes.selectedId ? notes.draft : note.body)}</span>
          </button>
        </li>
      {/each}
    </ul>
  </aside>
  <section class="editor">
    {#if notes.selected}
      <textarea
        value={notes.draft}
        oninput={(e) => notes.edit(e.currentTarget.value)}
        onfocus={() => void focusIsland()}
        placeholder="Write anything. It saves as you type."
        spellcheck="true"
      ></textarea>
      <footer>
        <span class="muted">{notes.saving ? "Saving…" : "Saved"}</span>
        <span class="tools">
          <button aria-label={notes.selected.pinned ? "Unpin" : "Pin"} class:on={notes.selected.pinned}
            onclick={() => notes.selected && void notes.togglePin(notes.selected)}><Icon name="pin" size={14} /></button>
          <button aria-label="Delete note" onclick={() => notes.selected && void notes.remove(notes.selected)}><Icon name="trash" size={14} /></button>
        </span>
      </footer>
    {:else}
      <p class="empty">Quick notes and checklists, saved as you type.</p>
    {/if}
  </section>
</div>

<style>
  .jot { height: 100%; display: grid; grid-template-columns: 170px 1fr; gap: 10px; min-height: 0; }
  aside { display: flex; flex-direction: column; gap: 6px; min-height: 0; }
  .new {
    display: flex; align-items: center; gap: 6px; justify-content: center;
    height: 32px; border: 0; border-radius: 10px;
    background: var(--surface-2); color: var(--fg); font: inherit; font-size: 12.5px; cursor: pointer;
  }
  .new:hover { background: var(--glow, var(--accent)); color: #fff; }
  ul { list-style: none; margin: 0; padding: 0; overflow-y: auto; flex: 1; scrollbar-width: thin; }
  .title {
    width: 100%; display: flex; align-items: center; gap: 6px;
    padding: 7px 10px; border: 0; border-radius: 8px;
    background: transparent; color: var(--muted); font: inherit; font-size: 13px; text-align: left; cursor: pointer;
  }
  .title span { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .title:hover { background: var(--surface-1); color: var(--fg); }
  .title.active { background: var(--surface-2); color: var(--fg); }
  .editor {
    display: flex; flex-direction: column; min-height: 0;
    border-radius: 14px; background: var(--surface-1); box-shadow: inset 0 0 0 1px var(--hairline);
  }
  textarea {
    flex: 1; resize: none; border: 0; outline: 0; background: transparent;
    color: var(--fg); font: inherit; font-size: 13.5px; line-height: 1.5;
    padding: 12px 14px; user-select: text; -webkit-user-select: text;
  }
  footer { display: flex; justify-content: space-between; align-items: center; padding: 4px 8px 6px 14px; }
  .muted { color: var(--muted); font-size: 11.5px; }
  .tools { display: flex; gap: 2px; }
  .tools button {
    width: 26px; height: 26px; display: grid; place-items: center;
    border: 0; border-radius: 7px; background: transparent; color: var(--muted); cursor: pointer;
  }
  .tools button:hover { background: var(--surface-2); color: var(--fg); }
  .tools button.on { color: var(--glow, var(--accent)); }
  .empty { margin: auto; color: var(--muted); font-size: 13px; }
</style>
