import type { NoteSummary, Status, Tag } from "./api";
import { StatusDot, TagChip } from "./NoteMeta";

interface Props {
  notebookName: string | null;
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

export function NoteList({
  notebookName,
  notes,
  statuses,
  tags,
  selectedId,
  onSelect,
  onCreate,
  onDelete,
}: Props) {
  const statusById = new Map(statuses.map((s) => [s.id, s]));
  const tagById = new Map(tags.map((t) => [t.id, t]));
  return (
    <section className="note-list">
      <div className="tree-header">
        <span className="ellipsis">{notebookName ?? "Notes"}</span>
        {notebookName && (
          <button className="icon-btn" title="New note" onClick={onCreate}>
            +
          </button>
        )}
      </div>
      {!notebookName && <div className="empty">Select a notebook</div>}
      {notebookName && notes.length === 0 && (
        <div className="empty">No notes. Click + to create one.</div>
      )}
      {notes.map((n) => (
        <div
          key={n.id}
          className={`note-row ${selectedId === n.id ? "selected" : ""}`}
          onClick={() => onSelect(n.id)}
        >
          <div className="note-row-top">
            <StatusDot status={n.statusId ? statusById.get(n.statusId) : undefined} />
            <span className="note-title ellipsis">{n.title || "Untitled"}</span>
            <button
              className="icon-btn del"
              title="Delete note"
              onClick={(e) => {
                e.stopPropagation();
                onDelete(n.id, n.title || "Untitled");
              }}
            >
              ×
            </button>
          </div>
          <div className="note-meta">
            <span>{formatDate(n.updatedAt)}</span>
            <span className="ellipsis">{plainSnippet(n.snippet)}</span>
          </div>
          {n.tagIds.length > 0 && (
            <div className="note-tags">
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
