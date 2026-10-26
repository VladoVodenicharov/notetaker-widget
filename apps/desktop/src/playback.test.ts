import { describe, expect, it } from "vitest";
import { canControl, IDLE_RESUME_MS, idlePause, onTick, onTyped } from "./playback";
import type { Source } from "./sources";

const video = (over: Partial<Source> = {}): Source => ({
  key: "youtube:intro",
  platform: "youtube",
  title: "Intro",
  channel: null,
  isPlaying: true,
  isMeeting: false,
  app: "chrome.exe",
  isFocused: false,
  focusedAgoMs: null,
  ...over,
});
const meet = video({ key: "meet:abc", platform: "meet", isMeeting: true });

describe("play and pause control", () => {
  it("is available for videos, not meetings or no source", () => {
    expect(canControl(video())).toBe(true);
    expect(canControl(meet)).toBe(false);
    expect(canControl(null)).toBe(false);
  });
});

describe("pause while typing", () => {
  it("does nothing when the feature is off", () => {
    expect(onTyped(idlePause, 1000, false, video()).send).toBeNull();
  });

  it("pauses a playing video on the first keystroke, once", () => {
    const first = onTyped(idlePause, 1000, true, video());
    expect(first.send).toEqual({ action: "pause", source: video() });
    expect(onTyped(first.state, 1200, true, video({ isPlaying: false })).send).toBeNull();
  });

  it("never pauses a meeting (Meet, Zoom, Teams) or a video that is not playing", () => {
    expect(onTyped(idlePause, 1000, true, meet).send).toBeNull();
    expect(onTyped(idlePause, 1000, true, video({ isPlaying: false })).send).toBeNull();
    expect(onTyped(idlePause, 1000, true, null).send).toBeNull();
  });

  it("resumes only after the idle wait, measured from the last keystroke", () => {
    let { state } = onTyped(idlePause, 1000, true, video());
    state = onTyped(state, 4000, true, video({ isPlaying: false })).state;
    expect(onTick(state, 4000 + IDLE_RESUME_MS - 1, false).send).toBeNull();
    const resumed = onTick(state, 4000 + IDLE_RESUME_MS, false);
    expect(resumed.send).toEqual({ action: "play", source: video() });
    expect(onTick(resumed.state, 99_000, false).send).toBeNull();
  });

  it("does not resume a video the user already started again", () => {
    const { state } = onTyped(idlePause, 1000, true, video());
    const step = onTick(state, 60_000, true);
    expect(step.send).toBeNull();
    expect(step.state.paused).toBeNull();
  });
});
