import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ChevronDown, ChevronRight, FileClock, X } from "lucide-react";
import { continueNoteAt, pickNoteToContinue, useSession } from "../session";

type Suggestion = { path: string; title: string };

/** "Continue a note…" before writing starts, or the note being continued with a way back out. */
export function ContinueChip() {
  const { continuing, startedAt, stopContinuing } = useSession();
  if (continuing) {
    return (
      <span className="chip max-w-full" title={continuing.path}>
        <FileClock size={14} />
        <span className="truncate">Adding to {continuing.title}</span>
        <button className="hover:text-ink" title="Start a new note instead" onClick={stopContinuing}>
          <X size={12} />
        </button>
      </span>
    );
  }
  // Mid-session the Finish sheet still offers "Add to existing".
  if (startedAt) return null;
  return (
    <button className="chip hover:bg-hover" onClick={() => void pickNoteToContinue()} title="Add today's notes to the end of a note you saved before">
      <FileClock size={14} /> Continue a note…
    </button>
  );
}

/** "Continue 'PostgreSQL Window Functions'?" when the current source was used in a saved note. */
export function ContinueSuggestion() {
  const { source, continuing, startedAt } = useSession();
  const [suggestion, setSuggestion] = useState<Suggestion | null>(null);
  const [dismissed, setDismissed] = useState<string | null>(null);
  const key = source?.key ?? null;

  useEffect(() => {
    setSuggestion(null);
    if (!key) return;
    let cancelled = false;
    invoke<Suggestion | null>("suggest_note", { keys: [key] }).then((s) => !cancelled && setSuggestion(s), console.error);
    return () => {
      cancelled = true;
    };
  }, [key]);

  if (!suggestion || continuing || startedAt || dismissed === suggestion.path) return null;
  return (
    <div className="mx-6 mb-2 flex items-center gap-2 rounded-lg bg-hover px-3 py-2 text-sm">
      <FileClock size={14} className="shrink-0 text-muted" />
      <button className="min-w-0 flex-1 truncate text-left text-ink hover:underline" onClick={() => void continueNoteAt(suggestion.path)} title={suggestion.path}>
        Continue “{suggestion.title}”?
      </button>
      <button className="text-muted hover:text-ink" title="No, start a new note" onClick={() => setDismissed(suggestion.path)}>
        <X size={14} />
      </button>
    </div>
  );
}

/** The end of the note being continued, read-only, so you can see where you left off. */
export function PreviouslyBox() {
  const continuing = useSession((s) => s.continuing);
  const [open, setOpen] = useState(true);
  if (!continuing) return null;
  return (
    <div className="mx-6 mb-2 rounded-lg border border-line">
      <button className="flex w-full items-center gap-1 px-3 py-1.5 text-sm text-muted hover:text-ink" onClick={() => setOpen((v) => !v)}>
        {open ? <ChevronDown size={14} /> : <ChevronRight size={14} />} Previously
      </button>
      {open && (
        <pre className="max-h-40 overflow-y-auto whitespace-pre-wrap border-t border-line px-3 py-2 font-sans text-xs leading-relaxed text-muted">
          {continuing.tail || "(empty note)"}
        </pre>
      )}
    </div>
  );
}
