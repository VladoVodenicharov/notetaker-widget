import { Extension } from "@tiptap/core";
import { Plugin, TextSelection, type EditorState, type Transaction } from "@tiptap/pm/state";
import type { SectionInsert } from "../sourceSwitching";

type Options = {
  /** Returns (and consumes) the heading waiting for the next keystroke, if any. */
  take: () => SectionInsert | null;
};

/** The heading(s) plus the typed `text` at `from`, with the cursor after the text. */
export function insertSection(state: EditorState, from: number, text: string, insert: SectionInsert): Transaction {
  const { schema } = state;
  const heading = (title: string) => schema.nodes.heading.create({ level: 3 }, title ? schema.text(title) : null);
  const paragraph = schema.nodes.paragraph.create(null, schema.text(text));
  const tr = state.tr;
  if (insert.first) tr.insert(0, heading(insert.first.title));

  const $at = tr.doc.resolve(tr.mapping.map(from));
  const next = heading(insert.next.title);
  let start: number;
  if ($at.depth === 1 && $at.parent.type.name === "paragraph" && $at.parent.content.size === 0) {
    // On an empty line: it becomes the heading and the typed text goes below it.
    start = $at.before(1);
    tr.replaceWith(start, $at.after(1), [next, paragraph]);
  } else {
    // Inside text, a list or code: the new section starts after that top-level block.
    start = $at.after(1);
    tr.insert(start, [next, paragraph]);
  }
  return tr.setSelection(TextSelection.create(tr.doc, start + next.nodeSize + 1 + text.length));
}

/**
 * After a source switch, the first character typed starts a `### <source title>` section.
 * On the first switch of a session, the earlier notes get a heading for the first source too.
 */
export const SourceSection = Extension.create<Options>({
  name: "sourceSection",
  addOptions() {
    return { take: () => null };
  },
  addProseMirrorPlugins() {
    const { take } = this.options;
    return [
      new Plugin({
        props: {
          handleTextInput: (view, from, to, text) => {
            if (from !== to) return false;
            const insert = take();
            if (!insert) return false;
            view.dispatch(insertSection(view.state, from, text, insert).scrollIntoView());
            return true;
          },
        },
      }),
    ];
  },
});
