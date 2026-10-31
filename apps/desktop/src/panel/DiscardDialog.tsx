import type { JSONContent } from "@tiptap/react";
import { noteTitle, wordCount } from "../note";

type Props = {
  title: string;
  doc: JSONContent;
  onKeep: () => void;
  onDiscard: () => void;
};

/** Confirms throwing away the note being written. A note being continued keeps its file; only the new text goes. */
export function DiscardDialog({ title, doc, onKeep, onDiscard }: Props) {
  return (
    <div className="absolute inset-0 z-20 flex items-center bg-black/20 p-4" onClick={onKeep}>
      <div
        role="dialog"
        className="w-full space-y-4 rounded-2xl border border-line bg-surface p-6 shadow-xl"
        onClick={(e) => e.stopPropagation()}
        onKeyDown={(e) => e.key === "Escape" && onKeep()}
      >
        <p className="text-ink">
          Discard “{noteTitle(title)}” ({wordCount(doc)} words)? This can't be undone.
        </p>
        <div className="flex justify-end gap-2">
          <button className="btn" onClick={onDiscard}>
            Discard
          </button>
          <button className="btn-primary" autoFocus onClick={onKeep}>
            Keep writing
          </button>
        </div>
      </div>
    </div>
  );
}
