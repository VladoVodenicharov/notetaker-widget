import type { JSONContent } from "@tiptap/core";
import { MarkdownManager } from "@tiptap/markdown";
import { extensions } from "./editor/extensions";

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

export type NoteInfo = {
  id: string;
  title: string;
  startedAt: Date;
  endedAt: Date;
};

const pad = (n: number) => String(n).padStart(2, "0");
export const ymd = (d: Date) => `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
export const hm = (d: Date) => `${pad(d.getHours())}:${pad(d.getMinutes())}`;

export const noteTitle = (title: string) => title.trim() || "Untitled";
export const defaultFileName = (title: string, date: Date) => `${noteTitle(title)} – ${ymd(date)}`;

/**
 * The saved file: a small YAML header (hidden by VS Code's preview), the title,
 * and one dated section per session. Continuing a note later appends another `##` section.
 */
export function buildNote(note: NoteInfo, body: string): string {
  const title = noteTitle(note.title);
  const header = [
    "---",
    `title: ${JSON.stringify(title)}`, // a JSON string is a valid YAML double-quoted scalar
    `created: ${ymd(note.startedAt)}T${hm(note.startedAt)}`,
    `updated: ${ymd(note.endedAt)}T${hm(note.endedAt)}`,
    "sessions: 1",
    `notetaker_id: ${note.id}`,
    "---",
  ].join("\n");
  const session = `## ${ymd(note.startedAt)} (${hm(note.startedAt)}–${hm(note.endedAt)})`;
  return `${header}\n\n# ${title}\n\n${session}\n\n${body.trim()}\n`;
}
