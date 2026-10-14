import { Extension } from "@tiptap/core";
import { Plugin } from "@tiptap/pm/state";
import StarterKit from "@tiptap/starter-kit";
import { TaskItem, TaskList } from "@tiptap/extension-list";
import { Placeholder } from "@tiptap/extensions";
import { CodeBlockLowlight } from "@tiptap/extension-code-block-lowlight";
import { Markdown } from "@tiptap/markdown";
import { common, createLowlight } from "lowlight";

/** Headings, lists, tasks, fences or quotes at the start of a line. */
export const MARKDOWN_BLOCK = /^(#{1,6} |\s*[-*+] |\s*\d+[.)] |```|> )/m;

/** Plain-text pastes (copied notes, chat answers) arrive as raw Markdown; render them. */
const MarkdownPaste = Extension.create({
  name: "markdownPaste",
  addProseMirrorPlugins() {
    const editor = this.editor;
    return [
      new Plugin({
        props: {
          handlePaste: (_view, event) => {
            const text = event.clipboardData?.getData("text/plain");
            if (!text || editor.isActive("codeBlock")) return false;
            if (!MARKDOWN_BLOCK.test(text)) return false;
            return editor.commands.insertContent(text, { contentType: "markdown" });
          },
        },
      }),
    ];
  },
});

/** Shared by the editor and by Markdown serialization (tests, draft recovery). */
export const extensions = [
  StarterKit.configure({
    codeBlock: false, // replaced by the highlighted version below
    underline: false, // no Markdown equivalent
    link: { openOnClick: false },
  }),
  TaskList,
  TaskItem.configure({ nested: true }),
  CodeBlockLowlight.configure({ lowlight: createLowlight(common) }),
  Placeholder.configure({ placeholder: "Write your notes here…" }),
  Markdown,
  MarkdownPaste,
];
