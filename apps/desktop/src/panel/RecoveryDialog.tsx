import type { Draft } from "../autosave";
import { hm, noteTitle, wordCount } from "../note";

type Props = {
  draft: Draft;
  onResume: () => void;
  onSaveNow: () => void;
  onDiscard: () => void;
};

/** Shown at launch when the last session ended without saving (crash, power cut, kill). */
export function RecoveryDialog({ draft, onResume, onSaveNow, onDiscard }: Props) {
  return (
    <div className="absolute inset-0 z-20 flex items-center bg-black/20 p-4">
      <div role="dialog" className="w-full space-y-4 rounded-2xl border border-line bg-surface p-6 shadow-xl">
        <p className="text-ink">
          Recover “{noteTitle(draft.title)}” ({wordCount(draft.doc)} words, last edit {hm(new Date(draft.savedAt))})?
        </p>
        <div className="flex justify-end gap-2">
          <button className="btn" onClick={onDiscard}>
            Discard
          </button>
          <button className="btn" onClick={onSaveNow}>
            Save now
          </button>
          <button className="btn-primary" autoFocus onClick={onResume}>
            Resume
          </button>
        </div>
      </div>
    </div>
  );
}
