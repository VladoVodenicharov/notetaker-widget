// Play / pause rules for the video being watched, and "pause while I type".
// Kept free of Tauri imports so it can be unit-tested; the live wiring is in usePlayback.ts.

import type { Source } from "./sources";

/** Typing pauses the video; it resumes after this long without a keystroke. */
export const IDLE_RESUME_MS = 5000;

/** Play and pause only make sense for a video, not a meeting. */
export const canControl = (source: Source | null): source is Source => source !== null && !source.isMeeting;

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
