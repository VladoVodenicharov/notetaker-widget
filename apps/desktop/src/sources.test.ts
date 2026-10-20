import { describe, expect, it } from "vitest";
import { followResult, sourceLabel, winnerOf, type Snapshot, type Source } from "./sources";

const source = (over: Partial<Source> = {}): Source => ({
  key: "youtube:intro",
  platform: "youtube",
  title: "Intro to SQL",
  channel: "DataLemur",
  isPlaying: true,
  isMeeting: false,
  app: "chrome.exe",
  isFocused: false,
  focusedAgoMs: null,
  ...over,
});

const idle = { pinned: false, started: false, titleEdited: false, title: "" };

describe("sourceLabel", () => {
  it("prefers the channel, then the platform", () => {
    expect(sourceLabel(source())).toBe("DataLemur");
    expect(sourceLabel(source({ channel: null }))).toBe("YouTube");
    expect(sourceLabel(source({ channel: null, platform: "teams" }))).toBe("Teams");
  });
});

describe("winnerOf", () => {
  it("finds the winner by key, or null", () => {
    const a = source({ key: "a" });
    const snapshot: Snapshot = { candidates: [a, source({ key: "b" })], winner: "a" };
    expect(winnerOf(snapshot)).toBe(a);
    expect(winnerOf({ candidates: [a], winner: null })).toBeNull();
    expect(winnerOf({ candidates: [], winner: "gone" })).toBeNull();
  });
});

describe("followResult", () => {
  it("fills the source and the empty title from the winner", () => {
    const winner = source();
    expect(followResult(idle, winner)).toEqual({ source: winner, title: "Intro to SQL" });
  });

  it("clears an untouched title when the source disappears", () => {
    expect(followResult({ ...idle, title: "Intro to SQL" }, null)).toEqual({ source: null, title: "" });
  });

  it("never overwrites a title the user typed", () => {
    const winner = source();
    expect(followResult({ ...idle, titleEdited: true, title: "My own title" }, winner)).toEqual({ source: winner, title: "My own title" });
  });

  it("stops following once the user chose by hand or started writing", () => {
    expect(followResult({ ...idle, pinned: true }, source())).toBeNull();
    expect(followResult({ ...idle, started: true }, source())).toBeNull();
  });
});
