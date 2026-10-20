import { CalendarDays, Folder } from "lucide-react";
import { pickFolder, useSession } from "../session";
import type { Snapshot } from "../sources";
import { SourceChip } from "./SourceChip";
import { ContinueChip } from "./Continue";

const folderName = (path: string) => path.split(/[\\/]/).filter(Boolean).pop() ?? path;

export function Header({ sources }: { sources: Snapshot }) {
  const { title, folder, startedAt, setTitle } = useSession();
  const date = new Date(startedAt ?? Date.now()).toLocaleDateString(undefined, { day: "numeric", month: "short" });

  return (
    <header className="px-6 pt-2 pb-3">
      <input
        className="w-full bg-transparent font-serif text-[32px] leading-tight text-ink outline-none placeholder:text-faint"
        placeholder="Untitled"
        value={title}
        onChange={(e) => setTitle(e.target.value)}
      />
      <div className="mt-3 flex flex-wrap gap-2">
        <span className="chip">
          <CalendarDays size={14} /> {date}
        </span>
        <SourceChip snapshot={sources} />
        <button className="chip hover:bg-hover" onClick={() => void pickFolder()} title={folder ?? "Choose a folder"}>
          <Folder size={14} /> {folder ? folderName(folder) : "Add folder"}
        </button>
        <ContinueChip />
      </div>
    </header>
  );
}
