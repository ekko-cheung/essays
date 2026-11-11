import { useState } from "react";
import type { Status, Tag } from "./api";
import { Icon } from "./Icon";

export function TagChip({ tag, onRemove }: { tag: Tag; onRemove?: () => void }) {
  return (
    <span className="chip" style={{ ["--c" as string]: tag.color }}>
      {tag.name}
      {onRemove && (
        <button title="Remove tag" onClick={onRemove}>
          <Icon name="x" size={12} />
        </button>
      )}
    </span>
  );
}

export function StatusDot({ status }: { status: Status | undefined }) {
  return (
    <span
      className="status-dot"
      title={status?.name ?? "No status"}
      style={{ background: status?.color ?? "transparent", borderColor: status?.color ?? "var(--muted)" }}
    />
  );
}

interface Props {
  statuses: Status[];
  tags: Tag[];
  statusId: string | null;
  tagIds: string[];
  onStatusChange: (id: string | null) => void;
  onTagsChange: (ids: string[]) => void;
  /** Create a tag by name (or return the existing one). */
  onCreateTag: (name: string) => Promise<Tag>;
}

export function NoteMeta(p: Props) {
  const [draft, setDraft] = useState("");
  const tagById = new Map(p.tags.map((t) => [t.id, t]));
  // Ignore ids of tags/statuses that were deleted while this note was open.
  const attached = p.tagIds.map((id) => tagById.get(id)).filter((t): t is Tag => !!t);
  const status = p.statuses.find((s) => s.id === p.statusId);

  const addTag = async () => {
    const name = draft.trim();
    if (!name) return;
    setDraft("");
    const existing = p.tags.find((t) => t.name.toLowerCase() === name.toLowerCase());
    const tag = existing ?? (await p.onCreateTag(name));
    if (!attached.some((t) => t.id === tag.id)) {
      p.onTagsChange([...attached.map((t) => t.id), tag.id]);
    }
  };

  return (
    <div className="note-meta-bar">
      <label className="status-select">
        <StatusDot status={status} />
        <select
          value={status ? status.id : ""}
          onChange={(e) => p.onStatusChange(e.target.value || null)}
        >
          <option value="">No status</option>
          {p.statuses.map((s) => (
            <option key={s.id} value={s.id}>
              {s.name}
            </option>
          ))}
        </select>
      </label>
      <div className="tag-area">
        <Icon name="tag" size={14} className="tag-area-icon" />
        {attached.map((t) => (
          <TagChip
            key={t.id}
            tag={t}
            onRemove={() => p.onTagsChange(attached.filter((x) => x.id !== t.id).map((x) => x.id))}
          />
        ))}
        <input
          className="tag-input"
          list="tag-suggestions"
          value={draft}
          placeholder="Add tag…"
          onChange={(e) => setDraft(e.target.value)}
          onKeyDown={(e) => {
            // Don't swallow Enter while an IME composition is in progress.
            if (e.key === "Enter" && !e.nativeEvent.isComposing) {
              e.preventDefault();
              void addTag();
            }
            if (e.key === "Backspace" && !draft && attached.length) {
              p.onTagsChange(attached.slice(0, -1).map((t) => t.id));
            }
          }}
        />
        <datalist id="tag-suggestions">
          {p.tags
            .filter((t) => !attached.some((a) => a.id === t.id))
            .map((t) => (
              <option key={t.id} value={t.name} />
            ))}
        </datalist>
      </div>
    </div>
  );
}
