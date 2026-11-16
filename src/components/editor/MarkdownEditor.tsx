import { useEffect, useRef } from "react";
import { Compartment, EditorState } from "@codemirror/state";
import { EditorView, keymap, drawSelection, placeholder } from "@codemirror/view";
import { defaultKeymap, history, historyKeymap, indentWithTab } from "@codemirror/commands";
import { markdown, markdownLanguage } from "@codemirror/lang-markdown";
import { languages } from "@codemirror/language-data";
import { HighlightStyle, syntaxHighlighting } from "@codemirror/language";
import { tags as t } from "@lezer/highlight";
import { autoCloseExtensions } from "./autoClose";
import { livePreview } from "./livePreview";

// Colors come from CSS variables so light/dark follow the app theme.
const highlight = HighlightStyle.define([
  { tag: t.heading1, fontSize: "1.5em", fontWeight: "700" },
  { tag: t.heading2, fontSize: "1.3em", fontWeight: "700" },
  { tag: t.heading3, fontSize: "1.15em", fontWeight: "700" },
  { tag: [t.heading4, t.heading5, t.heading6], fontWeight: "700" },
  { tag: t.strong, fontWeight: "700" },
  { tag: t.emphasis, fontStyle: "italic" },
  { tag: t.strikethrough, textDecoration: "line-through" },
  { tag: [t.link, t.url], color: "var(--accent)" },
  { tag: t.monospace, fontFamily: "var(--mono)", color: "var(--code-fg)" },
  { tag: [t.processingInstruction, t.contentSeparator, t.quote], color: "var(--muted)" },
  { tag: t.keyword, color: "var(--tok-keyword)" },
  { tag: [t.string, t.regexp], color: "var(--tok-string)" },
  { tag: [t.number, t.bool, t.null], color: "var(--tok-number)" },
  { tag: t.comment, color: "var(--muted)", fontStyle: "italic" },
  { tag: [t.function(t.variableName), t.propertyName], color: "var(--tok-fn)" },
  { tag: [t.typeName, t.className], color: "var(--tok-type)" },
]);

const theme = EditorView.theme({
  "&": { height: "100%", color: "var(--fg)", backgroundColor: "transparent" },
  "&.cm-focused": { outline: "none" },
  ".cm-scroller": { fontFamily: "var(--mono)", lineHeight: "1.65", padding: "8px 0" },
  ".cm-content": { caretColor: "var(--fg)", padding: "0 16px" },
  ".cm-cursor": { borderLeftColor: "var(--fg)" },
  ".cm-selectionBackground, &.cm-focused .cm-selectionBackground, ::selection": {
    backgroundColor: "color-mix(in srgb, var(--accent) 30%, transparent) !important",
  },
  ".cm-placeholder": { color: "var(--muted)" },
});

interface Props {
  initialValue: string;
  /** Render Markdown inline (Typora-style) instead of showing raw syntax. */
  live?: boolean;
  /** Insert matching closers for brackets and fenced code blocks. */
  autoClose?: boolean;
  onChange: (value: string) => void;
}

/** Uncontrolled: owns the document after mount. Remount (via `key`) to load another note. */
export function MarkdownEditor({ initialValue, live = false, autoClose = true, onChange }: Props) {
  const host = useRef<HTMLDivElement>(null);
  const editor = useRef<EditorView | null>(null);
  const autoCloseCompartment = useRef(new Compartment());
  const onChangeRef = useRef(onChange);
  onChangeRef.current = onChange;

  useEffect(() => {
    const view = new EditorView({
      parent: host.current!,
      state: EditorState.create({
        doc: initialValue,
        extensions: [
          history(),
          drawSelection(),
          EditorView.lineWrapping,
          placeholder("Write Markdown…"),
          keymap.of([...defaultKeymap, ...historyKeymap, indentWithTab]),
          markdown({ base: markdownLanguage, codeLanguages: languages }),
          syntaxHighlighting(highlight),
          theme,
          autoCloseCompartment.current.of(autoCloseExtensions(autoClose)),
          ...(live ? livePreview : []),
          EditorView.updateListener.of((u) => {
            if (u.docChanged) onChangeRef.current(u.state.doc.toString());
          }),
        ],
      }),
    });
    editor.current = view;
    return () => {
      editor.current = null;
      view.destroy();
    };
    // initialValue intentionally read once: the editor owns the doc afterwards.
    // live is fixed by the remount key in NoteEditor.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // The setting can change while a note is open; reconfigure without a remount.
  useEffect(() => {
    editor.current?.dispatch({
      effects: autoCloseCompartment.current.reconfigure(autoCloseExtensions(autoClose)),
    });
  }, [autoClose]);

  return <div className="cm-host" ref={host} />;
}
