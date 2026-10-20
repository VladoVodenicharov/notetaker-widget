import { useEffect, useRef, useState } from "react";
import { Check, ChevronDown, CirclePlay, GraduationCap, Globe, PenLine, Pin, PinOff, Users, Video } from "lucide-react";
import { useSession } from "../session";
import { platformLabel, sourceLabel, type Platform, type Snapshot, type Source } from "../sources";

const icons: Record<Platform, typeof Video> = {
  youtube: CirclePlay,
  datacamp: GraduationCap,
  meet: Video,
  zoom: Video,
  teams: Users,
  web: Globe,
};

function PlatformIcon({ platform, size = 14 }: { platform: Platform; size?: number }) {
  const Icon = icons[platform];
  return <Icon size={size} />;
}

/** What is being watched, with a list of the other detected sources and a manual option. */
export function SourceChip({ snapshot }: { snapshot: Snapshot }) {
  const { source, pinned, startedAt, chooseSource, setPinned } = useSession();
  const [open, setOpen] = useState(false);
  const root = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const away = (e: MouseEvent) => !root.current?.contains(e.target as Node) && setOpen(false);
    const esc = (e: KeyboardEvent) => e.key === "Escape" && setOpen(false);
    window.addEventListener("mousedown", away);
    window.addEventListener("keydown", esc);
    return () => {
      window.removeEventListener("mousedown", away);
      window.removeEventListener("keydown", esc);
    };
  }, [open]);

  const pick = (s: Source | null) => {
    chooseSource(s);
    setOpen(false);
  };

  return (
    <div ref={root} className="relative">
      <button className="chip hover:bg-hover" onClick={() => setOpen((v) => !v)} title={source?.title ?? "Choose what you are watching"}>
        {source ? <PlatformIcon platform={source.platform} /> : <PenLine size={14} />}
        {pinned && source && <Pin size={12} className="text-faint" />}
        <span className="max-w-[10rem] truncate">{source ? sourceLabel(source) : pinned ? "No source" : "Looking for a source…"}</span>
        <ChevronDown size={12} />
      </button>
      {open && (
        <ul className="absolute left-0 z-20 mt-1 w-72 overflow-hidden rounded-lg border border-line bg-surface py-1 text-sm shadow-lg">
          {snapshot.candidates.length === 0 && <li className="px-3 py-2 text-faint">No video or meeting found</li>}
          {snapshot.candidates.map((c) => (
            <li key={c.key}>
              <button className="flex w-full items-start gap-2 px-3 py-2 text-left hover:bg-hover" onClick={() => pick(c)}>
                <span className="mt-0.5 text-muted">
                  <PlatformIcon platform={c.platform} />
                </span>
                <span className="min-w-0 flex-1">
                  <span className="block truncate text-ink">{c.title}</span>
                  <span className="block truncate text-xs text-muted">
                    {[platformLabel[c.platform], c.channel, c.isPlaying ? "playing" : null].filter(Boolean).join(" · ")}
                  </span>
                </span>
                {source?.key === c.key && <Check size={14} className="mt-0.5 text-muted" />}
              </button>
            </li>
          ))}
          <li className="mt-1 border-t border-line pt-1">
            <button className="flex w-full items-center gap-2 px-3 py-2 text-left text-muted hover:bg-hover" onClick={() => pick(null)}>
              <PenLine size={14} /> Manual (no source)
              {pinned && !source && <Check size={14} className="ml-auto" />}
            </button>
          </li>
          <li>
            <button
              className="flex w-full items-center gap-2 px-3 py-2 text-left text-muted hover:bg-hover"
              title={startedAt ? "While pinned, switching to another video or meeting doesn't start a new section" : "Keep this source even if another one starts playing"}
              onClick={() => {
                setPinned(!pinned);
                setOpen(false);
              }}
            >
              {pinned ? <PinOff size={14} /> : <Pin size={14} />} {pinned ? "Unpin: follow what I watch" : "Pin this source"}
            </button>
          </li>
        </ul>
      )}
    </div>
  );
}
