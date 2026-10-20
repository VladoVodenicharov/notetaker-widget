// Switching sources in the middle of a session. Pure, so it is tested with a fake clock.
import type { Source } from "./sources";

/** A new source must stay the likely one this long before the session switches to it. */
export const STABLE_MS = 8000;

export type Switcher = {
  current: Source | null;
  /** A different source that is waiting out STABLE_MS. */
  pending: { source: Source; since: number } | null;
};

/** Only a source that is playing or in front counts: a paused video in a background tab doesn't. */
const eligible = (s: Source | null) => (s && (s.isPlaying || s.isFocused) ? s : null);

/** Feed the detected winner (on every change and once a second); returns the new state. */
export function step(state: Switcher, winner: Source | null, now: number): Switcher {
  const next = eligible(winner);
  if (!next) return state.pending ? { ...state, pending: null } : state;
  // Same source: keep it (its title or channel may have been filled in) and drop any pending switch.
  if (next.key === state.current?.key) return { current: next, pending: null };
  const since = state.pending?.source.key === next.key ? state.pending.since : now;
  if (now - since >= STABLE_MS) return { current: next, pending: null };
  return { current: state.current, pending: { source: next, since } };
}

/**
 * Section headers. After a switch, the new source's `###` heading is added the next time
 * the user types, so flicking between tabs leaves no empty headings.
 */
export type Sections = {
  /** The source the text being written belongs to. */
  writingKey: string | null;
  /** Switched to this source; its heading goes in on the next keystroke. */
  pending: Source | null;
  /** Some heading was already added (the first source then got one too). */
  labelled: boolean;
};

export const freshSections = (source: Source | null): Sections => ({ writingKey: source?.key ?? null, pending: null, labelled: false });

export function onSwitch(sections: Sections, source: Source | null): Sections {
  // Back to the source being written about before any typing: nothing to add.
  if (!source || source.key === sections.writingKey) return { ...sections, pending: null };
  return { ...sections, pending: source };
}

export type SectionInsert = {
  /** On the first switch, the first source's heading goes at the top so its notes are labelled too. */
  first: Pick<Source, "title"> | null;
  next: Source;
};

export function takePending(sections: Sections, first: Pick<Source, "title"> | null): { sections: Sections; insert: SectionInsert | null } {
  if (!sections.pending) return { sections, insert: null };
  return {
    sections: { writingKey: sections.pending.key, pending: null, labelled: true },
    insert: { first: sections.labelled ? null : first, next: sections.pending },
  };
}
