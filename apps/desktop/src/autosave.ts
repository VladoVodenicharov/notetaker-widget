import { useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { Editor, JSONContent } from "@tiptap/react";
import { useSession, type NoteSummary, type Segment } from "./session";
import type { Source } from "./sources";

/** The unfinished note in `drafts/draft.json`: editor content plus what's needed to rebuild the session. */
export type Draft = {
  id: string;
  title: string;
  folder: string | null;
  startedAt: number | null;
  savedAt: number;
  doc: JSONContent;
  // Added in M5; older drafts don't have them.
  source?: Source | null;
  segments?: Segment[];
  continuing?: NoteSummary | null;
};

export const loadDraft = () => invoke<Draft | null>("load_draft");

// Draft calls run one after another, so a delete is never overtaken by an older save.
let queue: Promise<unknown> = Promise.resolve();
const enqueue = (task: () => Promise<unknown>) => {
  queue = queue.then(task).catch((e) => console.error("draft:", e));
};

let source: Editor | null = null; // null while autosave is off (e.g. a recovery prompt is open)
let dirty = false;
let debounce: ReturnType<typeof setTimeout> | undefined;
let maxWait: ReturnType<typeof setTimeout> | undefined;

function clearTimers() {
  clearTimeout(debounce);
  clearTimeout(maxWait);
  maxWait = undefined;
}

/** Saves 750 ms after the last edit, and at least every 5 s while edits keep coming. */
function schedule() {
  if (!source) return;
  dirty = true;
  clearTimeout(debounce);
  debounce = setTimeout(flush, 750);
  maxWait ??= setTimeout(flush, 5000);
}

/** Writes the pending edits now. An empty editor means there's nothing worth recovering. */
function flush() {
  clearTimers();
  if (!dirty || !source) return;
  dirty = false;
  if (source.isEmpty) return enqueue(() => invoke("delete_draft"));
  const { id, title, folder, startedAt, source: from, segments, continuing } = useSession.getState();
  const draft: Draft = { id, title, folder, startedAt, savedAt: Date.now(), doc: source.getJSON(), source: from, segments, continuing };
  enqueue(() => invoke("save_draft", { draft }));
}

/** After the note is saved, or when a recovered draft is thrown away. */
export function discardDraft() {
  clearTimers();
  dirty = false;
  enqueue(() => invoke("delete_draft"));
}

/** Autosaves `editor` and the session's title and folder while `enabled`. */
export function useAutosave(editor: Editor | null, enabled: boolean) {
  useEffect(() => {
    if (!editor || !enabled) return;
    source = editor;
    editor.on("update", schedule);
    const unsubscribe = useSession.subscribe((s, prev) => {
      if (s.title !== prev.title || s.folder !== prev.folder || s.continuing !== prev.continuing) schedule();
    });
    window.addEventListener("blur", flush);
    const unlisten = getCurrentWindow().onCloseRequested(async () => {
      flush();
      await queue;
    });
    return () => {
      flush();
      source = null;
      editor.off("update", schedule);
      unsubscribe();
      window.removeEventListener("blur", flush);
      void unlisten.then((f) => f());
    };
  }, [editor, enabled]);
}
