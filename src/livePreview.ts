import { syntaxTree } from "@codemirror/language";
import type { EditorState, Range } from "@codemirror/state";
import { Decoration, type DecorationSet, EditorView, ViewPlugin, type ViewUpdate, WidgetType } from "@codemirror/view";

/**
 * Typora-style live preview: Markdown is rendered as you write, and the syntax
 * marks (`**`, `#`, `[](…)`, …) only show for the element the cursor is in.
 */

class RuleWidget extends WidgetType {
  toDOM() {
    const hr = document.createElement("div");
    hr.className = "cm-live-hr";
    return hr;
  }
  eq() {
    return true;
  }
}

class BulletWidget extends WidgetType {
  toDOM() {
    const el = document.createElement("span");
    el.className = "cm-live-bullet";
    el.textContent = "•";
    return el;
  }
  eq() {
    return true;
  }
}

class CheckboxWidget extends WidgetType {
  constructor(readonly checked: boolean) {
    super();
  }
  eq(o: CheckboxWidget) {
    return o.checked === this.checked;
  }
  toDOM(view: EditorView) {
    const box = document.createElement("input");
    box.type = "checkbox";
    box.className = "cm-live-check";
    box.checked = this.checked;
    // Toggle the `[ ]` / `[x]` in the document.
    box.addEventListener("mousedown", (e) => {
      e.preventDefault();
      const pos = view.posAtDOM(box);
      view.dispatch({ changes: { from: pos + 1, to: pos + 2, insert: this.checked ? " " : "x" } });
    });
    return box;
  }
  ignoreEvent() {
    return true;
  }
}

class ImageWidget extends WidgetType {
  constructor(readonly src: string, readonly alt: string) {
    super();
  }
  eq(o: ImageWidget) {
    return o.src === this.src && o.alt === this.alt;
  }
  toDOM() {
    const img = document.createElement("img");
    img.className = "cm-live-img";
    img.src = this.src;
    img.alt = this.alt;
    img.addEventListener("error", () => {
      const span = document.createElement("span");
      span.className = "cm-live-img-missing";
      span.textContent = `🖼 ${this.alt || this.src}`;
      img.replaceWith(span);
    });
    return img;
  }
}

const bullet = Decoration.replace({ widget: new BulletWidget() });
const hide = Decoration.replace({});
const rule = Decoration.replace({ widget: new RuleWidget() });
const lineDeco = (cls: string) => Decoration.line({ class: cls });
const markDeco = (cls: string) => Decoration.mark({ class: cls });

const HEADING = /^ATXHeading([1-6])$/;
const SETEXT = /^SetextHeading([12])$/;

function touches(state: EditorState, from: number, to: number): boolean {
  return state.selection.ranges.some((r) => r.from <= to && r.to >= from);
}

/** Selection touches any line the range spans. */
function touchesLines(state: EditorState, from: number, to: number): boolean {
  const a = state.doc.lineAt(from).from;
  const b = state.doc.lineAt(to).to;
  return touches(state, a, b);
}

function build(view: EditorView): DecorationSet {
  const { state } = view;
  const out: Range<Decoration>[] = [];
  const tree = syntaxTree(state);

  for (const { from, to } of view.visibleRanges) {
    tree.iterate({
      from,
      to,
      enter: (node) => {
        const name = node.name;
        let m: RegExpExecArray | null;

        if ((m = HEADING.exec(name) ?? SETEXT.exec(name))) {
          const first = state.doc.lineAt(node.from);
          const last = state.doc.lineAt(node.to);
          for (let n = first.number; n <= last.number; n++) {
            out.push(lineDeco(`cm-live-h cm-live-h${m[1]}`).range(state.doc.line(n).from));
          }
          return;
        }

        if (name === "FencedCode" || name === "CodeBlock") {
          const first = state.doc.lineAt(node.from).number;
          const last = state.doc.lineAt(node.to).number;
          for (let n = first; n <= last; n++) {
            const cls = `cm-live-code${n === first ? " cm-live-code-first" : ""}${n === last ? " cm-live-code-last" : ""}`;
            out.push(lineDeco(cls).range(state.doc.line(n).from));
          }
          return false; // leave code content untouched
        }

        if (name === "Blockquote") {
          const first = state.doc.lineAt(node.from).number;
          const last = state.doc.lineAt(node.to).number;
          for (let n = first; n <= last; n++) out.push(lineDeco("cm-live-quote").range(state.doc.line(n).from));
          return;
        }

        if (name === "InlineCode") {
          out.push(markDeco("cm-live-inline-code").range(node.from, node.to));
          return;
        }

        if (name === "StrongEmphasis") out.push(markDeco("cm-live-strong").range(node.from, node.to));
        else if (name === "Emphasis") out.push(markDeco("cm-live-em").range(node.from, node.to));
        else if (name === "Strikethrough") out.push(markDeco("cm-live-strike").range(node.from, node.to));

        if (name === "Link") {
          // [text](url): show only the text, link-styled, until the cursor enters it.
          const hasUrl = node.node.getChild("URL") !== null;
          if (hasUrl) {
            out.push(markDeco("cm-live-link").range(node.from, node.to));
            if (!touches(state, node.from, node.to)) {
              for (let c = node.node.firstChild; c; c = c.nextSibling) {
                if (c.name === "LinkMark" || c.name === "URL" || c.name === "LinkTitle") {
                  out.push(hide.range(c.from, c.to));
                }
              }
            }
          }
          return;
        }

        if (name === "Image") {
          const url = node.node.getChild("URL");
          if (url && !touches(state, node.from, node.to)) {
            const src = state.doc.sliceString(url.from, url.to);
            const text = state.doc.sliceString(node.from, node.to);
            const alt = /^!\[([^\]]*)\]/.exec(text)?.[1] ?? "";
            out.push(Decoration.replace({ widget: new ImageWidget(src, alt) }).range(node.from, node.to));
          }
          return false;
        }

        if (name === "Table") {
          const first = state.doc.lineAt(node.from).number;
          const last = state.doc.lineAt(node.to).number;
          const inside = touchesLines(state, node.from, node.to);
          for (let n = first; n <= last; n++) {
            const isHead = n === first;
            const isDelim = n === first + 1;
            const cls = `cm-live-table${isHead ? " cm-live-table-head" : ""}${
              isDelim && !inside ? " cm-live-table-delim" : ""
            }`;
            out.push(lineDeco(cls).range(state.doc.line(n).from));
          }
          return;
        }
        if (name === "TableDelimiter") {
          out.push(markDeco("cm-live-pipe").range(node.from, node.to));
          return;
        }

        if (name === "TaskMarker") {
          if (!touchesLines(state, node.from, node.from)) {
            const checked = /x/i.test(state.doc.sliceString(node.from, node.to));
            out.push(Decoration.replace({ widget: new CheckboxWidget(checked) }).range(node.from, node.to));
          }
          return;
        }

        if (name === "ListMark") {
          const item = node.node.parent;
          const list = item?.parent;
          const line = state.doc.lineAt(node.from);
          const isTask = item?.getChild("Task") != null;
          if (list?.name === "BulletList") {
            if (!touchesLines(state, node.from, node.from)) {
              if (isTask) {
                // The checkbox stands in for the bullet.
                const end = state.doc.sliceString(node.to, node.to + 1) === " " ? node.to + 1 : node.to;
                out.push(hide.range(node.from, end));
              } else {
                out.push(bullet.range(node.from, node.to));
              }
            }
          } else {
            out.push(markDeco("cm-live-olmark").range(node.from, node.to));
          }
          out.push(lineDeco("cm-live-li").range(line.from));
          return;
        }

        if (name === "HorizontalRule") {
          if (!touchesLines(state, node.from, node.to)) out.push(rule.range(node.from, node.to));
          return;
        }

        // Syntax marks: hidden unless the cursor is in their element.
        const parent = node.node.parent;
        if (!parent) return;
        if (name === "EmphasisMark" || name === "StrikethroughMark") {
          if (!touches(state, parent.from, parent.to)) out.push(hide.range(node.from, node.to));
        } else if (name === "CodeMark" && parent.name === "InlineCode") {
          if (!touches(state, parent.from, parent.to)) out.push(hide.range(node.from, node.to));
        } else if (name === "HeaderMark" && HEADING.test(parent.name)) {
          if (!touchesLines(state, node.from, node.from)) {
            // Also swallow the space after the #'s.
            const end = state.doc.sliceString(node.to, node.to + 1) === " " ? node.to + 1 : node.to;
            out.push(hide.range(node.from, end));
          }
        } else if (name === "QuoteMark") {
          if (!touchesLines(state, node.from, node.from)) {
            const end = state.doc.sliceString(node.to, node.to + 1) === " " ? node.to + 1 : node.to;
            out.push(hide.range(node.from, end));
          }
        }
      },
    });
  }
  return Decoration.set(out, true);
}

export const livePreview = [
  ViewPlugin.fromClass(
    class {
      decorations: DecorationSet;
      constructor(view: EditorView) {
        this.decorations = build(view);
      }
      update(u: ViewUpdate) {
        if (u.docChanged || u.selectionSet || u.viewportChanged || syntaxTree(u.startState) !== syntaxTree(u.state)) {
          this.decorations = build(u.view);
        }
      }
    },
    { decorations: (v) => v.decorations },
  ),
  // Rendered look: proportional font, roomier lines. Scoped so other modes are unaffected.
  EditorView.theme({
    "&": { fontFamily: "var(--sans)" },
    ".cm-scroller": { fontFamily: "var(--sans)", lineHeight: "1.75" },
    ".cm-content": { maxWidth: "820px", margin: "0 auto", fontSize: "15px" },
    ".cm-live-h": { fontWeight: "700", lineHeight: "1.4" },
    ".cm-live-h1": { fontSize: "1.9em" },
    ".cm-live-h2": { fontSize: "1.5em" },
    ".cm-live-h3": { fontSize: "1.25em" },
    ".cm-live-h4, .cm-live-h5, .cm-live-h6": { fontSize: "1.08em" },
    ".cm-live-strong": { fontWeight: "700" },
    ".cm-live-em": { fontStyle: "italic" },
    ".cm-live-strike": { textDecoration: "line-through", color: "var(--muted)" },
    ".cm-live-link": { color: "var(--accent)", textDecoration: "underline" },
    ".cm-live-inline-code": {
      fontFamily: "var(--mono)",
      fontSize: "0.88em",
      background: "var(--code-bg)",
      color: "var(--code-fg)",
      padding: "0.1em 0.3em",
      borderRadius: "4px",
    },
    ".cm-live-quote": { borderLeft: "3px solid var(--border)", paddingLeft: "12px !important", color: "var(--fg-2)" },
    ".cm-live-code": {
      fontFamily: "var(--mono)",
      fontSize: "0.9em",
      background: "var(--code-bg)",
      paddingLeft: "14px !important",
    },
    ".cm-live-code-first": { borderTopLeftRadius: "8px", borderTopRightRadius: "8px" },
    ".cm-live-code-last": { borderBottomLeftRadius: "8px", borderBottomRightRadius: "8px" },
    ".cm-live-bullet": { display: "inline-block", width: "1em", textAlign: "center", color: "var(--muted)" },
    ".cm-live-olmark": { color: "var(--muted)" },
    ".cm-live-check": { margin: "0 6px 0 0", verticalAlign: "middle", accentColor: "var(--accent)", cursor: "pointer" },
    ".cm-live-img": { maxWidth: "100%", borderRadius: "6px", verticalAlign: "middle" },
    ".cm-live-img-missing": { color: "var(--muted)" },
    ".cm-live-table": { fontFamily: "var(--mono)", fontSize: "0.9em", borderBottom: "1px solid var(--border)" },
    ".cm-live-table-head": { fontWeight: "700", background: "var(--hover)" },
    ".cm-live-table-delim": { fontSize: "2px", lineHeight: "2px", color: "transparent", height: "2px", overflow: "hidden" },
    ".cm-live-pipe": { color: "var(--muted)" },
    ".cm-live-hr": { display: "inline-block", width: "100%", borderTop: "1px solid var(--border)", verticalAlign: "middle" },
  }),
];
