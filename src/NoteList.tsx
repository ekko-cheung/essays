import type { ReactNode } from "react";
import type { NoteSummary, Status, Tag } from "./api";
import { StatusDot, TagChip } from "./NoteMeta";

interface Props {
  title: string;
  /** Notes can only be created inside a concrete notebook, not in "All Notes". */
  canCreate: boolean;
  /** Show each note's notebook (when the list spans several notebooks). */
  notebookNameById: Map<string, string> | null;
  /** True when a search or filter is narrowing the list (changes the empty message). */
  narrowed: boolean;
  filterBar: ReactNode;
  notes: NoteSummary[];
  statuses: Status[];
  tags: Tag[];
  selectedId: string | null;
  onSelect: (id: string) => void;
  onCreate: () => void;
  onDelete: (id: string, title: string) => void;
}

function formatDate(ms: number) {
  const d = new Date(ms);
  const now = new Date();
  return d.toDateString() === now.toDateString()
    ? d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })
    : d.toLocaleDateString();
}

// Strip the most common Markdown syntax so the one-line preview reads naturally.
function plainSnippet(md: string) {
  return md
    .replace(/```[\s\S]*?```/g, " ")
    .replace(/[#>*_`~\-\[\]()!|]/g, "")
    .replace(/\s+/g, " ")
    .trim()
    .slice(0, 90);
}

export function NoteList(p: Props) {
  const statusById = new Map(p.statuses.map((s) => [s.id, s]));
  const tagById = new Map(p.tags.map((t) => [t.id, t]));
  return (
    <section className="note-list">
      <div className="tree-header">
        <span className="ellipsis">{p.title}</span>
        <span className="count">{p.notes.length}</span>
        {p.canCreate && (
          <button className="icon-btn" title="New note" onClick={p.onCreate}>
            +
          </button>
        )}
      </div>
      {p.filterBar}
      {p.notes.length === 0 && (
        <div className="empty">
          {p.narrowed
            ? "No notes match the current search or filters."
            : p.canCreate
              ? "No notes. Click + to create one."
              : "No notes yet."}
        </div>
      )}
      {p.notes.map((n) => (
        <div
          key={n.id}
          className={`note-row ${p.selectedId === n.id ? "selected" : ""}`}
          onClick={() => p.onSelect(n.id)}
        >
          <div className="note-row-top">
            <StatusDot status={n.statusId ? statusById.get(n.statusId) : undefined} />
            <span className="note-title ellipsis">{n.title || "Untitled"}</span>
            <button
              className="icon-btn del"
              title="Delete note"
              onClick={(e) => {
                e.stopPropagation();
                p.onDelete(n.id, n.title || "Untitled");
              }}
            >
              ×
            </button>
          </div>
          <div className="note-meta">
            <span>{formatDate(n.updatedAt)}</span>
            <span className="ellipsis">{plainSnippet(n.snippet)}</span>
          </div>
          {(n.tagIds.length > 0 || p.notebookNameById) && (
            <div className="note-tags">
              {p.notebookNameById && (
                <span className="nb-name">{p.notebookNameById.get(n.notebookId) ?? "?"}</span>
              )}
              {n.tagIds.map((id) => {
                const t = tagById.get(id);
                return t ? <TagChip key={id} tag={t} /> : null;
              })}
            </div>
          )}
        </div>
      ))}
    </section>
  );
}
