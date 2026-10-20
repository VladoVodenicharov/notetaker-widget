import type { JSONContent } from "@tiptap/core";
import { MarkdownManager } from "@tiptap/markdown";
import { extensions } from "./editor/extensions";
import { sourceRef, type Source } from "./sources";

const markdown = new MarkdownManager({ extensions });

/** Editor JSON to Markdown. Empty paragraphs are just spacing; Markdown would write them as "&nbsp;". */
export function toMarkdown(doc: JSONContent): string {
  const content = doc.content?.filter((node) => node.type !== "paragraph" || node.content?.length);
  return markdown.serialize({ ...doc, content });
}

/** Words in editor JSON. Inline pieces (bold, links) join up; blocks are separated by spaces. */
export function wordCount(doc: JSONContent): number {
  const text = (node: JSONContent): string => node.text ?? ` ${(node.content ?? []).map(text).join("")} `;
  return text(doc).split(/\s+/).filter(Boolean).length;
}

export type SourceInfo = Pick<Source, "platform" | "title" | "channel">;

export type NoteInfo = {
  id: string;
  title: string;
  startedAt: Date;
  endedAt: Date;
  /** The sources of this session, in order. The first one is the note's main source. */
  sources?: SourceInfo[];
};

const pad = (n: number) => String(n).padStart(2, "0");
export const ymd = (d: Date) => `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
export const hm = (d: Date) => `${pad(d.getHours())}:${pad(d.getMinutes())}`;

export const noteTitle = (title: string) => title.trim() || "Untitled";
export const defaultFileName = (title: string, date: Date) => `${noteTitle(title)} – ${ymd(date)}`;

export const stamp = (d: Date) => `${ymd(d)}T${hm(d)}`;

/** The same source twice in a session (switched away and back) is listed once. */
export const sourceRefs = (sources: SourceInfo[] = []) => [...new Set(sources.map(sourceRef))];

/** One session: `## 2026-10-12 (19:05–20:10)` and its notes. Continuing a note appends exactly this. */
export const buildSession = (note: Pick<NoteInfo, "startedAt" | "endedAt">, body: string) =>
  `## ${ymd(note.startedAt)} (${hm(note.startedAt)}–${hm(note.endedAt)})\n\n${body.trim()}`;

/**
 * The saved file: a small YAML header (hidden by VS Code's preview), the title,
 * and one dated section per session. Continuing a note later appends another `##` section.
 */
export function buildNote(note: NoteInfo, body: string): string {
  const title = noteTitle(note.title);
  const refs = sourceRefs(note.sources);
  const main = note.sources?.[0];
  // A JSON string is a valid YAML double-quoted scalar.
  const header = [
    "---",
    `title: ${JSON.stringify(title)}`,
    `created: ${stamp(note.startedAt)}`,
    `updated: ${stamp(note.endedAt)}`,
    "sessions: 1",
    `notetaker_id: ${note.id}`,
    ...(main ? [`platform: ${main.platform}`] : []),
    ...(main?.channel ? [`channel: ${JSON.stringify(main.channel)}`] : []),
    ...(refs.length ? ["sources:", ...refs.map((r) => `  - ${JSON.stringify(r)}`)] : []),
    "---",
  ].join("\n");
  const sourceLine = refs.length ? `**${refs.length > 1 ? "Sources" : "Source"}:** ${refs.join("; ")}\n\n` : "";
  return `${header}\n\n# ${title}\n\n${sourceLine}${buildSession(note, body)}\n`;
}
