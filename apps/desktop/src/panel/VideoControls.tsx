import { Pause, Play } from "lucide-react";
import { canControl } from "../playback";
import type { Source } from "../sources";

/** Play / pause for the video being watched, and the "pause while I type" option. */
export function VideoControls({
  source,
  onToggle,
  pauseWhileTyping,
  onTogglePauseWhileTyping,
  lastFailure,
}: {
  source: Source | null;
  onToggle: () => void;
  pauseWhileTyping: boolean;
  onTogglePauseWhileTyping: () => void;
  /** Why the last play / pause didn't reach the player, if it didn't. */
  lastFailure: string | null;
}) {
  const enabled = canControl(source);
  const action = !source ? "No video to control" : source.isMeeting ? "Meetings can't be played or paused from here" : source.isPlaying ? "Pause (Ctrl+Alt+Space)" : "Play (Ctrl+Alt+Space)";
  const title = lastFailure ? `${action}\nLast command: ${lastFailure}` : action;
  return (
    <div className="flex items-center gap-3">
      <button className="icon-btn disabled:opacity-40 disabled:hover:bg-transparent" title={title} disabled={!enabled} onClick={onToggle}>
        {enabled && source.isPlaying ? <Pause size={18} /> : <Play size={18} />}
      </button>
      <label className="flex items-center gap-1.5 text-xs text-muted" title="Pauses the video while you type and resumes after a few seconds. Never for meetings.">
        <input type="checkbox" checked={pauseWhileTyping} onChange={onTogglePauseWhileTyping} />
        Pause while I type
      </label>
    </div>
  );
}
