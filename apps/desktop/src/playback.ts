// Play / pause rules for the video being watched, and "pause while I type".
// Kept free of Tauri imports so it can be unit-tested; the live wiring is in usePlayback.ts.

import type { Snapshot, Source } from "./sources";

/** Typing pauses the video; it resumes after this long without a keystroke. */
export const IDLE_RESUME_MS = 5000;

/** Play and pause only make sense for a video, not a meeting. */
export const canControl = (source: Source | null): source is Source => source !== null && !source.isMeeting;

/** Why a media command did or didn't reach the player (Rust `control::Reason`). */
export type MediaReason =
  | { kind: "noSession" }
  | { kind: "otherBrowser" }
  | { kind: "titleMismatch"; page: string; media: string }
  | { kind: "notPlaying" }
  | { kind: "matched" }
  | { kind: "refused" };

export type MediaResult = { ok: boolean; reason: MediaReason };

/** Text for the play button's tooltip when a command failed. */
export function describeFailure(reason: MediaReason): string {
  switch (reason.kind) {
    case "noSession":
      return "Windows reports no media playing";
    case "otherBrowser":
      return "No media session from this browser";
    case "titleMismatch":
      return `The browser's media "${reason.media}" doesn't match the page "${reason.page}"`;
    case "refused":
      return "The player refused the command";
    default:
      return "Unknown problem";
  }
}

/**
 * Is the source playing right now? Only the live snapshot knows; a source missing from it
 * (a paused Web page can drop out) counts as not playing, never as the stale copy's state.
 */
export const playingNow = (snapshot: Snapshot, source: Source): boolean =>
  snapshot.candidates.find((c) => c.key === source.key)?.isPlaying ?? false;

/** What pause-while-typing did last: the source it paused, if it still owes a resume. */
export type TypingPause = { paused: Source | null; lastTypedAt: number };

export const idlePause: TypingPause = { paused: null, lastTypedAt: 0 };

export type Step = { state: TypingPause; send: { action: "play" | "pause"; source: Source } | null };

/** A keystroke: pause the video if it is playing and the feature is on. Never for meetings. */
export function onTyped(state: TypingPause, now: number, enabled: boolean, source: Source | null): Step {
  const next = { ...state, lastTypedAt: now };
  if (!enabled || state.paused || !canControl(source) || !source.isPlaying) return { state: next, send: null };
  return { state: { paused: source, lastTypedAt: now }, send: { action: "pause", source } };
}

/**
 * Called about once a second: resume what we paused after the idle wait.
 * If the video is already playing again (the user pressed play) we just forget about it.
 */
export function onTick(state: TypingPause, now: number, playingNow: boolean | null): Step {
  if (!state.paused) return { state, send: null };
  if (playingNow) return { state: idlePause, send: null };
  if (now - state.lastTypedAt < IDLE_RESUME_MS) return { state, send: null };
  return { state: { ...state, paused: null }, send: { action: "play", source: state.paused } };
}
