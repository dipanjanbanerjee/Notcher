import { noteDelete, noteSave, notesList, type Note } from "../bridge";

/** First non-empty line, used as a note's title. */
export function noteTitle(body: string): string {
  const line = body.split("\n").find((l) => l.trim()) ?? "";
  return line.replace(/^\s*(?:[-*]\s*)?\[[ xX]\]\s*/, "").replace(/^#+\s*/, "").trim() || "Untitled";
}

/** Flips the checkbox on one line ("[ ] task" ⇄ "[x] task"). */
export function toggleCheck(body: string, lineIndex: number): string {
  const lines = body.split("\n");
  const line = lines[lineIndex];
  if (line === undefined) return body;
  lines[lineIndex] = line.replace(/\[( |x|X)\]/, (_m, mark: string) => (mark === " " ? "[x]" : "[ ]"));
  return lines.join("\n");
}

class NotesStore {
  items = $state<Note[]>([]);
  selectedId = $state<number | null>(null);
  draft = $state("");
  saving = $state(false);
  private timer: ReturnType<typeof setTimeout> | null = null;

  get selected(): Note | null {
    return this.items.find((n) => n.id === this.selectedId) ?? null;
  }

  async load() {
    const list = await notesList();
    if (!list) return;
    this.items = list;
    if (this.selectedId === null && list.length) this.select(list[0]);
  }

  select(note: Note) {
    void this.flush();
    this.selectedId = note.id;
    this.draft = note.body;
  }

  async create() {
    await this.flush();
    const note = await noteSave({ id: null, body: "", pinned: false });
    if (!note) return;
    this.items = [note, ...this.items];
    this.selectedId = note.id;
    this.draft = "";
  }

  /** Autosave: saves 400 ms after the last keystroke. */
  edit(body: string) {
    this.draft = body;
    if (this.timer) clearTimeout(this.timer);
    this.timer = setTimeout(() => void this.flush(), 400);
  }

  async flush() {
    if (this.timer) clearTimeout(this.timer);
    this.timer = null;
    const note = this.selected;
    if (!note || note.body === this.draft) return;
    this.saving = true;
    const saved = await noteSave({ id: note.id, body: this.draft, pinned: note.pinned });
    this.saving = false;
    if (saved) this.items = this.items.map((n) => (n.id === saved.id ? saved : n));
  }

  async togglePin(note: Note) {
    const saved = await noteSave({ id: note.id, body: note.id === this.selectedId ? this.draft : note.body, pinned: !note.pinned });
    if (saved) await this.load();
  }

  async remove(note: Note) {
    await noteDelete(note.id);
    this.items = this.items.filter((n) => n.id !== note.id);
    if (this.selectedId === note.id) {
      this.selectedId = null;
      this.draft = "";
      if (this.items.length) this.select(this.items[0]);
    }
  }
}

export const notes = new NotesStore();
