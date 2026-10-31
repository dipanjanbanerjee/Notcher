import { clipCopy, clipDelete, clipSetPinned, clipsClear, clipsList, onClipAdded, type Clip } from "../bridge";

/** Clipboard history shared by the Home and Clipboard tabs. */
class ClipsStore {
  items = $state<Clip[]>([]);
  query = $state("");
  /** Id of the clip most recently copied back, for a brief "Copied" tick. */
  justCopied = $state<number | null>(null);
  private searchSeq = 0;

  async refresh() {
    const seq = ++this.searchSeq;
    const list = await clipsList(this.query);
    if (seq === this.searchSeq && list) this.items = list;
  }

  /** Called for every new copy; keeps the list ordered without a round-trip. */
  added(clip: Clip) {
    if (this.query) {
      void this.refresh();
      return;
    }
    const rest = this.items.filter((c) => c.id !== clip.id);
    const pinned = rest.filter((c) => c.pinned);
    const unpinned = rest.filter((c) => !c.pinned);
    this.items = clip.pinned ? [clip, ...pinned, ...unpinned] : [...pinned, clip, ...unpinned];
  }

  async copy(clip: Clip) {
    await clipCopy(clip.id);
    this.justCopied = clip.id;
    setTimeout(() => {
      if (this.justCopied === clip.id) this.justCopied = null;
    }, 1200);
  }

  async togglePin(clip: Clip) {
    await clipSetPinned(clip.id, !clip.pinned);
    await this.refresh();
  }

  async remove(clip: Clip) {
    this.items = this.items.filter((c) => c.id !== clip.id);
    await clipDelete(clip.id);
  }

  async clear() {
    await clipsClear();
    await this.refresh();
  }

  listen(onNew?: (clip: Clip) => void) {
    return onClipAdded((clip) => {
      this.added(clip);
      onNew?.(clip);
    });
  }
}

export const clips = new ClipsStore();
