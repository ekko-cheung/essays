import { closeBrackets, closeBracketsKeymap } from "@codemirror/autocomplete";
import { syntaxTree } from "@codemirror/language";
import {
  EditorSelection,
  EditorState,
  Prec,
  StateEffect,
  StateField,
  type Extension,
} from "@codemirror/state";
import { EditorView, keymap, type Command } from "@codemirror/view";

/**
 * Auto-closing for Markdown editing.
 *
 * - `(`, `[`, and `{` insert their matching closer. Typing a closer that was
 *   auto-inserted moves over it instead of duplicating it, and Backspace
 *   between an auto-inserted pair removes both characters.
 * - A third backtick at the start of a line opens a fenced code block and
 *   inserts the closing fence below, with the cursor at the end of the opening
 *   line (between the fences). Backspace there removes the pair again.
 *
 * Quotes and single backticks are deliberately not auto-closed: in prose they
 * are usually literal characters, not delimiters.
 */
const BRACKETS = ["(", "[", "{"];

const addFence = StateEffect.define<number>();

/** Positions of closing fences inserted by this extension. */
const insertedFences = StateField.define<number[]>({
  create: () => [],
  update(fences, tr) {
    const doc = tr.state.doc;
    const kept = fences.map((pos) => tr.changes.mapPos(pos, 1)).filter((pos) => {
      const line = doc.lineAt(pos);
      return line.from === pos && line.text === "```";
    });
    for (const effect of tr.effects) {
      if (effect.is(addFence)) kept.push(effect.value);
    }
    return kept;
  },
});

/** True when `pos` is inside a fenced or indented code block. */
function insideCode(state: EditorState, pos: number): boolean {
  const start = syntaxTree(state).resolveInner(pos, -1);
  if (start.name === "FencedCode" || start.name === "CodeBlock") return true;
  for (let parent = start.parent; parent; parent = parent.parent) {
    if (parent.name === "FencedCode" || parent.name === "CodeBlock") return true;
  }
  return false;
}

const completeFence = EditorView.inputHandler.of((view, from, to, text) => {
  if (view.composing || view.compositionStarted || view.state.readOnly) return false;
  if (from !== to || text !== "`") return false;

  const state = view.state;
  const sel = state.selection.main;
  if (!sel.empty || sel.head !== from) return false;

  const line = state.doc.lineAt(from);
  // Only the third backtick of an otherwise empty, unindented line is a fence.
  if (!/^ {0,3}``$/.test(state.sliceDoc(line.from, from))) return false;
  if (state.sliceDoc(from, line.to).trim() !== "") return false;
  // Typing ``` inside an open block closes it; don't add a nested fence.
  if (insideCode(state, line.from)) return false;

  view.dispatch({
    changes: { from, insert: "`\n```" },
    // Cursor after the opening fence, before the newline: between the fences.
    selection: EditorSelection.cursor(from + 1),
    effects: addFence.of(from + 2),
    userEvent: "input.type",
    scrollIntoView: true,
  });
  return true;
});

/** Backspace directly after an auto-inserted opening fence removes both fences. */
const deleteFencePair: Command = ({ state, dispatch }) => {
  const sel = state.selection.main;
  if (!sel.empty) return false;

  const head = sel.head;
  const line = state.doc.lineAt(head);
  if (!/^ {0,3}```$/.test(state.sliceDoc(line.from, head))) return false;
  if (!state.field(insertedFences).includes(head + 1)) return false;

  dispatch(
    state.update({
      changes: { from: head - 1, to: head + 4 },
      selection: EditorSelection.cursor(head - 1),
      userEvent: "delete.backward",
      scrollIntoView: true,
    }),
  );
  return true;
};

/** Bracket and code-fence completion, or nothing when disabled in settings. */
export function autoCloseExtensions(enabled: boolean): Extension {
  if (!enabled) return [];
  return [
    EditorState.languageData.of(() => [{ closeBrackets: { brackets: BRACKETS } }]),
    closeBrackets(),
    insertedFences,
    completeFence,
    // Highest precedence so pair deletion wins over the default Backspace.
    Prec.highest(
      keymap.of([{ key: "Backspace", run: deleteFencePair }, ...closeBracketsKeymap]),
    ),
  ];
}
