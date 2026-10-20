import { describe, expect, it } from "vitest";
import { MarkdownManager } from "@tiptap/markdown";
import { buildNote, buildSession, defaultFileName, toMarkdown, wordCount } from "./note";
import { extensions, MARKDOWN_BLOCK } from "./editor/extensions";

const start = new Date(2026, 9, 13, 19, 42);
const end = new Date(2026, 9, 13, 20, 31);

describe("buildNote", () => {
  it("writes front matter, title and a dated session section", () => {
    const md = buildNote({ id: "abc", title: "Window Functions", startedAt: start, endedAt: end }, "- one\n");
    expect(md).toBe(
      [
        "---",
        'title: "Window Functions"',
        "created: 2026-10-13T19:42",
        "updated: 2026-10-13T20:31",
        "sessions: 1",
        "notetaker_id: abc",
        "---",
        "",
        "# Window Functions",
        "",
        "## 2026-10-13 (19:42–20:31)",
        "",
        "- one",
        "",
      ].join("\n"),
    );
  });

  it("escapes quotes in the YAML title and falls back to Untitled", () => {
    expect(buildNote({ id: "x", title: 'Say "hi": now', startedAt: start, endedAt: end }, "")).toContain(
      'title: "Say \\"hi\\": now"',
    );
    expect(buildNote({ id: "x", title: "  ", startedAt: start, endedAt: end }, "")).toContain("# Untitled");
  });
});

describe("buildNote with sources", () => {
  const yt = { platform: "youtube" as const, title: "Intro to SQL", channel: "DataLemur" };
  const teams = { platform: "teams" as const, title: "Standup", channel: null };

  it("records the main source in the front matter and under the title", () => {
    const md = buildNote({ id: "abc", title: "SQL", startedAt: start, endedAt: end, sources: [yt] }, "- one\n");
    expect(md).toContain(
      ["notetaker_id: abc", "platform: youtube", 'channel: "DataLemur"', "sources:", '  - "YouTube · Intro to SQL (DataLemur)"', "---"].join("\n"),
    );
    expect(md).toContain("# SQL\n\n**Source:** YouTube · Intro to SQL (DataLemur)\n\n## 2026-10-13 (19:42–20:31)");
  });

  it("lists every source once", () => {
    const md = buildNote({ id: "abc", title: "Mixed", startedAt: start, endedAt: end, sources: [yt, teams, yt] }, "x");
    expect(md).toContain(["sources:", '  - "YouTube · Intro to SQL (DataLemur)"', '  - "Teams · Standup"', "---"].join("\n"));
    expect(md).toContain("**Sources:** YouTube · Intro to SQL (DataLemur); Teams · Standup");
  });
});

describe("buildSession", () => {
  it("is the dated heading and the notes, ready to append", () => {
    expect(buildSession({ startedAt: start, endedAt: end }, "\n- two\n\n")).toBe("## 2026-10-13 (19:42–20:31)\n\n- two");
  });
});

describe("defaultFileName", () => {
  it("combines title and date", () => {
    expect(defaultFileName("SQL joins", start)).toBe("SQL joins – 2026-10-13");
    expect(defaultFileName("", start)).toBe("Untitled – 2026-10-13");
  });
});

describe("wordCount", () => {
  it("joins inline pieces and separates blocks", () => {
    const doc = {
      type: "doc",
      content: [
        { type: "paragraph", content: [{ type: "text", text: "Hello " }, { type: "text", text: "wor", marks: [{ type: "bold" }] }, { type: "text", text: "ld" }] },
        { type: "heading", attrs: { level: 2 }, content: [{ type: "text", text: "two" }] },
        { type: "paragraph", content: [{ type: "text", text: "words" }] },
        { type: "paragraph" },
      ],
    };
    expect(wordCount(doc)).toBe(4);
  });
});

describe("editor markdown", () => {
  const markdown = new MarkdownManager({ extensions });
  const text = (t: string) => ({ type: "text", text: t });
  const p = (t: string) => ({ type: "paragraph", content: [text(t)] });

  it("serializes the blocks people use in lecture notes", () => {
    const doc = {
      type: "doc",
      content: [
        { type: "heading", attrs: { level: 3 }, content: [text("Joins")] },
        {
          type: "bulletList",
          content: [
            { type: "listItem", content: [p("inner join"), { type: "bulletList", content: [{ type: "listItem", content: [p("matching rows only")] }] }] },
          ],
        },
        {
          type: "taskList",
          content: [{ type: "taskItem", attrs: { checked: false }, content: [p("redo exercise")] }],
        },
        { type: "codeBlock", attrs: { language: "sql" }, content: [text("SELECT *\nFROM a JOIN b USING (id);")] },
      ],
    };
    expect(toMarkdown(doc)).toBe(
      [
        "### Joins",
        "",
        "- inner join",
        "  - matching rows only",
        "",
        "- [ ] redo exercise",
        "",
        "```sql",
        "SELECT *",
        "FROM a JOIN b USING (id);",
        "```",
      ].join("\n"),
    );
  });

  it("treats only block syntax as pasted Markdown", () => {
    for (const md of ["### Joins", "- item", "intro\n1. first", "```sql\nx\n```", "> quote"]) {
      expect(MARKDOWN_BLOCK.test(md), md).toBe(true);
    }
    for (const plain of ["Just a dictated sentence.", "#hashtag", "costs -5 dollars", "3.14 is pi"]) {
      expect(MARKDOWN_BLOCK.test(plain), plain).toBe(false);
    }
  });

  it("round-trips its own output", () => {
    const md = "## Notes\n\n- a\n- b\n\n```python\nprint(1)\n```";
    expect(toMarkdown(markdown.parse(md))).toBe(md);
  });

  it("drops empty paragraphs instead of writing &nbsp;", () => {
    const doc = { type: "doc", content: [{ type: "paragraph" }, p("text"), { type: "paragraph" }, { type: "paragraph" }] };
    expect(toMarkdown(doc)).toBe("text");
  });
});
