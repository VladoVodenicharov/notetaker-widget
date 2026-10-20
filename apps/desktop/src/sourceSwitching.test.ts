import { describe, expect, it } from "vitest";
import { getSchema } from "@tiptap/core";
import { EditorState } from "@tiptap/pm/state";
import { MarkdownManager } from "@tiptap/markdown";
import { extensions } from "./editor/extensions";
import { insertSection } from "./editor/sourceSection";
import { freshSections, onSwitch, STABLE_MS, step, takePending, type Switcher } from "./sourceSwitching";
import type { Source } from "./sources";

const video = (key: string, over: Partial<Source> = {}): Source => ({
  key,
  platform: "youtube",
  title: key.toUpperCase(),
  channel: null,
  isPlaying: true,
  isMeeting: false,
  app: "chrome.exe",
  isFocused: false,
  focusedAgoMs: null,
  ...over,
});

describe("step (switching mid-session)", () => {
  const a = video("a");
  const b = video("b");
  const start: Switcher = { current: a, pending: null };

  it("switches only after the new source stays the likely one for 8 s", () => {
    let s = step(start, b, 0);
    expect(s.current).toBe(a);
    s = step(s, b, STABLE_MS - 1);
    expect(s.current).toBe(a);
    s = step(s, b, STABLE_MS);
    expect(s.current).toBe(b);
    expect(s.pending).toBeNull();
  });

  it("restarts the wait when the candidate changes, and forgets it when you come back", () => {
    let s = step(start, b, 0);
    s = step(s, video("c"), 5000);
    s = step(s, video("c"), 5000 + STABLE_MS - 1);
    expect(s.current).toBe(a);
    s = step(s, a, 14_000);
    expect(s).toEqual({ current: a, pending: null });
  });

  it("ignores a source that is neither playing nor in front", () => {
    const paused = video("b", { isPlaying: false });
    expect(step(step(start, paused, 0), paused, 60_000).current).toBe(a);
    const focused = video("b", { isPlaying: false, isFocused: true });
    expect(step(step(start, focused, 0), focused, STABLE_MS).current?.key).toBe("b");
  });

  it("takes in a refreshed copy of the same source", () => {
    const withChannel = video("a", { channel: "Chan" });
    expect(step(start, withChannel, 1).current).toBe(withChannel);
  });
});

describe("section headings", () => {
  const a = video("a");
  const b = video("b");

  it("adds a heading only after a real switch, and the first source's heading only once", () => {
    let sections = onSwitch(freshSections(a), b);
    let taken = takePending(sections, a);
    expect(taken.insert).toEqual({ first: a, next: b });
    sections = onSwitch(taken.sections, video("c"));
    taken = takePending(sections, a);
    expect(taken.insert?.first).toBeNull();
    expect(takePending(taken.sections, a).insert).toBeNull();
  });

  it("flicking away and back before typing adds nothing", () => {
    const sections = onSwitch(onSwitch(freshSections(a), b), a);
    expect(takePending(sections, a).insert).toBeNull();
  });

  it("inserts the headings into the document", () => {
    const schema = getSchema(extensions);
    const md = new MarkdownManager({ extensions });
    const doc = schema.nodeFromJSON({
      type: "doc",
      content: [
        { type: "paragraph", content: [{ type: "text", text: "about A" }] },
        { type: "paragraph" },
      ],
    });
    const state = EditorState.create({ schema, doc });
    // "x" typed on the empty second line.
    const from = doc.child(0).nodeSize + 1;
    const tr = insertSection(state, from, "x", { first: a, next: b });
    expect(md.serialize(tr.doc.toJSON())).toBe("### A\n\nabout A\n\n### B\n\nx");
    // Typed in the middle of a paragraph: the new section starts after it.
    const mid = insertSection(state, 3, "y", { first: null, next: b });
    expect(md.serialize(mid.doc.toJSON())).toBe("about A\n\n### B\n\ny\n\n");
  });
});
