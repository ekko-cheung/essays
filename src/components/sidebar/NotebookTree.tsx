import { useState } from "react";
import { Icon } from "../common/Icon";
import { ALL_NOTES, type TreeNode, type DropPosition } from "../../lib/tree";

interface Props {
  nodes: TreeNode[];
  selectedId: string | null;
  onSelect: (id: string) => void;
  onCreate: (parentId: string | null, name: string) => void;
  onRename: (id: string, name: string) => void;
  onDelete: (id: string, name: string) => void;
  onDrop: (dragId: string, targetId: string, pos: DropPosition) => void;
}

export function NotebookTree(props: Props) {
  // id of the node currently being renamed, or "new:<parentId|root>" for a new-child input
  const [editing, setEditing] = useState<string | null>(null);
  const [collapsed, setCollapsed] = useState<Set<string>>(new Set());
  const [dragId, setDragId] = useState<string | null>(null);
  const [over, setOver] = useState<{ id: string; pos: DropPosition } | null>(null);

  const toggle = (id: string) =>
    setCollapsed((s) => {
      const next = new Set(s);
      if (!next.delete(id)) next.add(id);
      return next;
    });

  const posFromEvent = (e: React.DragEvent<HTMLDivElement>): DropPosition => {
    const r = e.currentTarget.getBoundingClientRect();
    const y = (e.clientY - r.top) / r.height;
    return y < 0.25 ? "before" : y > 0.75 ? "after" : "inside";
  };

  const renderInput = (
    initial: string,
    onCommit: (v: string) => void,
    depth: number,
  ) => (
    <input
      className="tree-input"
      style={{ marginLeft: depth * 16 + 8 }}
      autoFocus
      defaultValue={initial}
      onBlur={(e) => {
        const v = e.target.value.trim();
        setEditing(null);
        if (v && v !== initial) onCommit(v);
      }}
      onKeyDown={(e) => {
        if (e.key === "Enter") e.currentTarget.blur();
        if (e.key === "Escape") {
          e.currentTarget.value = initial;
          e.currentTarget.blur();
        }
      }}
    />
  );

  const renderNodes = (nodes: TreeNode[], depth: number) =>
    nodes.map((n) => {
      const isCollapsed = collapsed.has(n.id);
      const cls = [
        "tree-row",
        props.selectedId === n.id ? "selected" : "",
        over?.id === n.id ? `drop-${over.pos}` : "",
      ].join(" ");
      return (
        <div key={n.id}>
          {editing === n.id ? (
            renderInput(n.name, (v) => props.onRename(n.id, v), depth)
          ) : (
            <div
              className={cls}
              style={{ paddingLeft: depth * 16 + 4 }}
              draggable
              onClick={() => props.onSelect(n.id)}
              onDoubleClick={() => setEditing(n.id)}
              onDragStart={(e) => {
                setDragId(n.id);
                e.dataTransfer.effectAllowed = "move";
              }}
              onDragEnd={() => {
                setDragId(null);
                setOver(null);
              }}
              onDragOver={(e) => {
                if (!dragId || dragId === n.id) return;
                e.preventDefault();
                const pos = posFromEvent(e);
                if (over?.id !== n.id || over.pos !== pos) setOver({ id: n.id, pos });
              }}
              onDragLeave={() => setOver((o) => (o?.id === n.id ? null : o))}
              onDrop={(e) => {
                e.preventDefault();
                if (dragId && dragId !== n.id) props.onDrop(dragId, n.id, posFromEvent(e));
                setDragId(null);
                setOver(null);
              }}
            >
              <span
                className="twisty"
                onClick={(e) => {
                  e.stopPropagation();
                  toggle(n.id);
                }}
              >
                {n.children.length > 0 && (
                  <Icon name="chevron" size={12} className={isCollapsed ? "" : "open"} />
                )}
              </span>
              <Icon name="book" size={15} className="row-icon" />
              <span className="name">{n.name}</span>
              <span className="actions">
                <button
                  title="New sub-notebook"
                  onClick={(e) => {
                    e.stopPropagation();
                    setCollapsed((s) => {
                      const next = new Set(s);
                      next.delete(n.id);
                      return next;
                    });
                    setEditing(`new:${n.id}`);
                  }}
                >
                  <Icon name="plus" size={14} />
                </button>
                <button
                  title="Delete"
                  onClick={(e) => {
                    e.stopPropagation();
                    props.onDelete(n.id, n.name);
                  }}
                >
                  <Icon name="trash" size={14} />
                </button>
              </span>
            </div>
          )}
          {editing === `new:${n.id}` &&
            renderInput("", (v) => props.onCreate(n.id, v), depth + 1)}
          {!isCollapsed && renderNodes(n.children, depth + 1)}
        </div>
      );
    });

  return (
    <div className="tree">
      <div className="tree-header">
        <span>Notebooks</span>
        <button className="icon-btn" title="New notebook" onClick={() => setEditing("new:root")}>
          <Icon name="plus" size={15} />
        </button>
      </div>
      <div
        className={`tree-row ${props.selectedId === ALL_NOTES ? "selected" : ""}`}
        style={{ paddingLeft: 4 }}
        onClick={() => props.onSelect(ALL_NOTES)}
      >
        <span className="twisty" />
        <Icon name="layers" size={15} className="row-icon" />
        <span className="name">All Notes</span>
      </div>
      {editing === "new:root" && renderInput("", (v) => props.onCreate(null, v), 0)}
      {renderNodes(props.nodes, 0)}
      {props.nodes.length === 0 && editing !== "new:root" && (
        <div className="empty">No notebooks yet. Click + to create one.</div>
      )}
    </div>
  );
}
