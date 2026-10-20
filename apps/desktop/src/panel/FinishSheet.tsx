import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { Editor } from "@tiptap/react";
import { buildNote, buildSession, defaultFileName, sourceRefs, stamp, toMarkdown, type SourceInfo } from "../note";
import { pickFolder, pickNoteToContinue, useSession } from "../session";

type Props = {
  editor: Editor;
  onClose: () => void;
  onSaved: (path: string) => void;
};

type Mode = "new" | "append";

export function FinishSheet({ editor, onClose, onSaved }: Props) {
  const session = useSession();
  const startedAt = new Date(session.startedAt ?? Date.now());
  const [mode, setMode] = useState<Mode>(session.continuing ? "append" : "new");
  const [name, setName] = useState(() => defaultFileName(session.title, startedAt));
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  // The segments, or the chosen source if writing never started with one.
  const sources: SourceInfo[] = session.segments.length ? session.segments : session.source ? [session.source] : [];
  const sourceKeys = [...new Set([...session.segments.map((s) => s.key), ...(session.source ? [session.source.key] : [])])];

  async function save() {
    setSaving(true);
    setError(null);
    try {
      const endedAt = new Date();
      const body = toMarkdown(editor.getJSON());
      let path: string;
      let title = session.title;
      let id: string | null = session.id;
      if (mode === "append") {
        const target = session.continuing ?? (await pickNoteToContinue());
        if (!target) return;
        path = await invoke<string>("append_session", {
          path: target.path,
          section: buildSession({ startedAt, endedAt }, body),
          updated: stamp(endedAt),
          sources: sourceRefs(sources),
        });
        title = target.title;
        id = target.id;
      } else {
        const folder = session.folder ?? (await pickFolder());
        if (!folder) return;
        const content = buildNote({ id: session.id, title: session.title, startedAt, endedAt, sources }, body);
        path = await invoke<string>("save_note", { folder, name, content });
      }
      // Only for "Continue…?" suggestions, so a failure here doesn't fail the save.
      void invoke("index_note", { entry: { path, title, id, sourceKeys, updated: endedAt.getTime() } }).catch(console.error);
      session.reset();
      onSaved(path);
    } catch (e) {
      setError(String(e));
    } finally {
      setSaving(false);
    }
  }

  const tab = (value: Mode, label: string) => (
    <button
      type="button"
      className={`rounded-md px-3 py-1.5 text-sm ${mode === value ? "bg-surface text-ink shadow-sm" : "text-muted hover:text-ink"}`}
      onClick={() => setMode(value)}
    >
      {label}
    </button>
  );

  return (
    <div className="absolute inset-0 z-10 flex flex-col justify-end bg-black/20" onClick={onClose}>
      <form
        className="space-y-4 rounded-t-2xl border-t border-line bg-surface p-6 shadow-xl"
        onClick={(e) => e.stopPropagation()}
        onKeyDown={(e) => e.key === "Escape" && onClose()}
        onSubmit={(e) => {
          e.preventDefault();
          void save();
        }}
      >
        <div className="flex items-center justify-between gap-2">
          <h2 className="text-lg font-medium text-ink">Save note</h2>
          <div className="flex rounded-lg bg-hover p-0.5">
            {tab("new", "New note")}
            {tab("append", "Add to existing")}
          </div>
        </div>

        {mode === "new" ? (
          <>
            <label className="block space-y-1">
              <span className="text-sm text-muted">Name</span>
              <input className="field" autoFocus value={name} onChange={(e) => setName(e.target.value)} />
            </label>
            <div className="space-y-1">
              <span className="text-sm text-muted">Folder</span>
              <div className="flex gap-2">
                <div className="field truncate text-muted" title={session.folder ?? ""}>
                  {session.folder ?? "No folder chosen yet"}
                </div>
                <button type="button" className="btn" onClick={() => void pickFolder()}>
                  Browse…
                </button>
              </div>
            </div>
          </>
        ) : (
          <div className="space-y-1">
            <span className="text-sm text-muted">Add a new dated section to the end of</span>
            <div className="flex gap-2">
              <div className="field truncate text-muted" title={session.continuing?.path ?? ""}>
                {session.continuing?.title ?? "No note chosen yet"}
              </div>
              <button type="button" className="btn" onClick={() => void pickNoteToContinue()}>
                Pick note…
              </button>
            </div>
          </div>
        )}

        {error && <p className="text-sm text-red-600">{error}</p>}
        <div className="flex justify-end gap-2">
          <button type="button" className="btn" onClick={onClose}>
            Cancel
          </button>
          <button type="submit" className="btn-primary" autoFocus={mode === "append"} disabled={saving || (mode === "new" && !name.trim())}>
            {saving ? "Saving…" : mode === "append" ? "Add to note" : "Save"}
          </button>
        </div>
      </form>
    </div>
  );
}
