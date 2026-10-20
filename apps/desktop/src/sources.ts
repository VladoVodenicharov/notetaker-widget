// What the panel knows about the video, course or meeting being watched.
// Kept free of Tauri imports so it can be unit-tested; the live wiring is in useFollowSource.ts.

export type Platform = "youtube" | "datacamp" | "meet" | "zoom" | "teams" | "web";

/** One candidate from the Rust side (`sources::Candidate`). */
export type Source = {
  key: string;
  platform: Platform;
  title: string;
  channel: string | null;
  isPlaying: boolean;
  isMeeting: boolean;
  app: string;
  /** Its window is in front right now (the panel itself doesn't count). */
  isFocused: boolean;
  focusedAgoMs: number | null;
};

export type Snapshot = {
  /** Best first. */
  candidates: Source[];
  winner: string | null;
};

export const emptySnapshot: Snapshot = { candidates: [], winner: null };

export const platformLabel: Record<Platform, string> = {
  youtube: "YouTube",
  datacamp: "DataCamp",
  meet: "Google Meet",
  zoom: "Zoom",
  teams: "Teams",
  web: "Web",
};

/** Short text for the chip: the channel when known, otherwise the platform. */
export const sourceLabel = (s: Source): string => s.channel || platformLabel[s.platform];

/** What a saved note records about a source: "YouTube · Intro to SQL (DataLemur)". */
export const sourceRef = (s: Pick<Source, "platform" | "title" | "channel">): string =>
  `${platformLabel[s.platform]} · ${s.title}${s.channel ? ` (${s.channel})` : ""}`;

export const winnerOf = (snapshot: Snapshot): Source | null =>
  snapshot.candidates.find((c) => c.key === snapshot.winner) ?? null;

type Header = { pinned: boolean; started: boolean; titleEdited: boolean; title: string };

/**
 * What the header should show when the detected sources change, or `null` to leave it alone.
 * It follows the winner only until the user picks a source by hand or starts writing;
 * a title the user typed is never overwritten.
 */
export function followResult(header: Header, winner: Source | null): { source: Source | null; title: string } | null {
  if (header.pinned || header.started) return null;
  return { source: winner, title: header.titleEdited ? header.title : (winner?.title ?? "") };
}
