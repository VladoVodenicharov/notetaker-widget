import { create } from "zustand";
import { LazyStore } from "@tauri-apps/plugin-store";
import { open } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";
import { followResult, type Platform, type Source } from "./sources";
import { freshSections, onSwitch, takePending, type SectionInsert, type Sections } from "./sourceSwitching";

const settings = new LazyStore("settings.json");

/** One stretch of the session spent on one source. Recorded when writing about it starts. */
export type Segment = { key: string; platform: Platform; title: string; channel: string | null; startedAt: number };

/** An existing note being continued (Rust `NoteSummary`). */
export type NoteSummary = { path: string; title: string; id: string | null; tail: string };

type Session = {
  id: string;
  title: string;
  /** True once the user typed the title, so a detected source never overwrites it. */
  titleEdited: boolean;
  /** What is being watched: detected automatically, or chosen by hand. */
  source: Source | null;
  /** The user picked a source (or "No source") by hand or pinned it, so detection stops changing it. */
  pinned: boolean;
  segments: Segment[];
  sections: Sections;
  /** Set when the new notes will be added to the end of an existing note. */
  continuing: NoteSummary | null;
  /** Where the next note is saved; remembered across sessions. */
  folder: string | null;
  /** Set on the first keystroke, so a panel left open all day doesn't skew the times. */
  startedAt: number | null;
  setTitle: (title: string) => void;
  /** The user's choice from the source chip; `null` is "No source". Mid-session it switches at once. */
  chooseSource: (source: Source | null) => void;
  setPinned: (pinned: boolean) => void;
  /** The detected winner changed: update the header unless the user took over or started writing. */
  follow: (winner: Source | null) => void;
  /** Mid-session switch (after the stability wait). */
  switchTo: (source: Source) => void;
  /** Called by the editor on a keystroke: the heading to insert first, if a switch is waiting. */
  takeSection: () => SectionInsert | null;
  continueNote: (note: NoteSummary) => void;
  stopContinuing: () => void;
  setFolder: (folder: string) => void;
  markStarted: () => void;
  reset: () => void;
};

const segmentOf = (s: Source, startedAt: number): Segment => ({ key: s.key, platform: s.platform, title: s.title, channel: s.channel, startedAt });

const blank = () => ({
  id: crypto.randomUUID(),
  title: "",
  titleEdited: false,
  source: null,
  pinned: false,
  segments: [],
  sections: freshSections(null),
  continuing: null,
  startedAt: null,
});

export const useSession = create<Session>()((set, get) => ({
  ...blank(),
  folder: null,
  setTitle: (title) => set({ title, titleEdited: true }),
  chooseSource: (source) => {
    if (get().startedAt !== null) {
      set((s) => ({ source, pinned: true, sections: onSwitch(s.sections, source) }));
      return;
    }
    set((s) => ({ source, pinned: true, title: s.titleEdited ? s.title : (source?.title ?? "") }));
  },
  setPinned: (pinned) => set({ pinned }),
  follow: (winner) => {
    const s = get();
    const next = followResult({ pinned: s.pinned, started: s.startedAt !== null, titleEdited: s.titleEdited, title: s.title }, winner);
    if (next && (next.source !== s.source || next.title !== s.title)) set(next);
  },
  switchTo: (source) => set((s) => ({ source, sections: onSwitch(s.sections, source) })),
  takeSection: () => {
    const s = get();
    const { sections, insert } = takePending(s.sections, s.segments[0] ?? null);
    if (!insert) return null;
    set({ sections, segments: [...s.segments, segmentOf(insert.next, Date.now())] });
    return insert;
  },
  continueNote: (note) => set({ continuing: note, title: note.title, titleEdited: true }),
  stopContinuing: () => set((s) => ({ continuing: null, title: s.source?.title ?? "", titleEdited: false })),
  setFolder: (folder) => {
    set({ folder });
    void settings.set("lastFolder", folder);
  },
  markStarted: () =>
    set((s) => {
      if (s.startedAt) return s;
      const now = Date.now();
      return { startedAt: now, segments: s.source ? [segmentOf(s.source, now)] : [], sections: freshSections(s.source) };
    }),
  reset: () => set(blank()),
}));

void settings.get<string>("lastFolder").then((folder) => {
  if (folder) useSession.setState((s) => (s.folder ? s : { folder }));
});

export async function pickFolder(): Promise<string | null> {
  const current = useSession.getState().folder;
  const picked = await open({ directory: true, defaultPath: current ?? undefined, title: "Folder for your notes" });
  if (typeof picked !== "string") return null;
  useSession.getState().setFolder(picked);
  return picked;
}

/** Opens a picker for an existing `.md` note and loads it for continuing. */
export async function pickNoteToContinue(): Promise<NoteSummary | null> {
  const { folder, continuing } = useSession.getState();
  const picked = await open({
    title: "Continue a note",
    defaultPath: continuing?.path ?? folder ?? undefined,
    filters: [{ name: "Markdown", extensions: ["md"] }],
  });
  if (typeof picked !== "string") return null;
  return continueNoteAt(picked);
}

export async function continueNoteAt(path: string): Promise<NoteSummary> {
  const note = await invoke<NoteSummary>("read_note", { path });
  useSession.getState().continueNote(note);
  return note;
}
