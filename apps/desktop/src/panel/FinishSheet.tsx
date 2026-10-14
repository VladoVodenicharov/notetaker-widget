import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { Editor } from "@tiptap/react";
import { buildNote, defaultFileName, toMarkdown } from "../note";
import { pickFolder, useSession } from "../session";

type Props = {
  editor: Editor;
  onClose: () => void;
  onSaved: (path: string) => void;
};

export function FinishSheet({ editor, onClose, onSaved }: Props) {
  const session = useSession();
  const startedAt = new Date(session.startedAt ?? Date.now());
  const [name, setName] = useState(() => defaultFileName(session.title, startedAt));
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  async function save() {
    const folder = session.folder ?? (await pickFolder());
    if (!folder) return;
    setSaving(true);
    setError(null);
    try {
      const content = buildNote(
        { id: session.id, title: session.title, startedAt, endedAt: new Date() },
        toMarkdown(editor.getJSON()),
      );
      const path = await invoke<string>("save_note", { folder, name, content });
      session.reset();
      onSaved(path);
    } catch (e) {
      setError(String(e));
    } finally {
      setSaving(false);
    }
  }

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
        <h2 className="text-lg font-medium text-ink">Save note</h2>
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
        {error && <p className="text-sm text-red-600">{error}</p>}
        <div className="flex justify-end gap-2">
          <button type="button" className="btn" onClick={onClose}>
            Cancel
          </button>
          <button type="submit" className="btn-primary" disabled={saving || !name.trim()}>
            {saving ? "Saving…" : "Save"}
          </button>
        </div>
      </form>
    </div>
  );
}
