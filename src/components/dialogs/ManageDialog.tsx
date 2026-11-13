import { useEffect, useRef, useState } from "react";
import { Icon } from "../common/Icon";

export interface Item {
  id: string;
  name: string;
  color: string;
  noteCount: number;
}

interface Props {
  title: string;
  noun: string; // "status" | "tag"
  items: Item[];
  defaultColor: string;
  /** All handlers may throw; the dialog shows the message and reverts the row. */
  onCreate: (name: string, color: string) => Promise<void>;
  onUpdate: (id: string, name: string, color: string) => Promise<void>;
  onDelete: (item: Item) => Promise<void>;
  onClose: () => void;
}

function Row({
  item,
  onUpdate,
  onDelete,
  onError,
}: {
  item: Item;
  onUpdate: Props["onUpdate"];
  onDelete: Props["onDelete"];
  onError: (m: string | null) => void;
}) {
  const [name, setName] = useState(item.name);
  const [color, setColor] = useState(item.color);
  const timer = useRef<number | undefined>(undefined);

  // Follow the server value whenever it changes (e.g. after a refresh).
  useEffect(() => {
    setName(item.name);
    setColor(item.color);
  }, [item.name, item.color]);
  useEffect(() => () => window.clearTimeout(timer.current), []);

  const commit = async (n: string, c: string) => {
    n = n.trim();
    if (n === item.name && c === item.color) return;
    try {
      onError(null);
      await onUpdate(item.id, n, c);
    } catch (e) {
      onError(String(e));
      setName(item.name);
      setColor(item.color);
    }
  };

  return (
    <div className="manage-row">
      <input
        type="color"
        value={color}
        onChange={(e) => {
          const c = e.target.value;
          setColor(c);
          // The picker fires continuously; save once it settles.
          window.clearTimeout(timer.current);
          timer.current = window.setTimeout(() => commit(name, c), 400);
        }}
      />
      <input
        className="manage-name"
        value={name}
        onChange={(e) => setName(e.target.value)}
        onBlur={() => commit(name, color)}
        onKeyDown={(e) => {
          if (e.key === "Enter") e.currentTarget.blur();
          if (e.key === "Escape") setName(item.name);
        }}
      />
      <span className="manage-count" title="Notes using it">
        {item.noteCount}
      </span>
      <button
        className="icon-btn"
        title="Delete"
        onClick={() => onDelete(item).catch((e) => onError(String(e)))}
      >
        <Icon name="trash" size={14} />
      </button>
    </div>
  );
}

export function ManageDialog(props: Props) {
  const [error, setError] = useState<string | null>(null);
  const [newName, setNewName] = useState("");
  const [newColor, setNewColor] = useState(props.defaultColor);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && props.onClose();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [props]);

  const add = async () => {
    if (!newName.trim()) return;
    try {
      setError(null);
      await props.onCreate(newName, newColor);
      setNewName("");
    } catch (e) {
      setError(String(e));
    }
  };

  return (
    <div className="overlay" onMouseDown={props.onClose}>
      <div className="dialog" onMouseDown={(e) => e.stopPropagation()}>
        <div className="dialog-head">
          <h3>{props.title}</h3>
          <button className="icon-btn" onClick={props.onClose}>
            <Icon name="x" size={16} />
          </button>
        </div>
        {error && <div className="error">{error}</div>}
        <div className="manage-list">
          {props.items.map((it) => (
            <Row
              key={it.id}
              item={it}
              onUpdate={props.onUpdate}
              onDelete={props.onDelete}
              onError={setError}
            />
          ))}
          {props.items.length === 0 && <div className="empty">No {props.noun}s yet.</div>}
        </div>
        <div className="manage-row add">
          <input type="color" value={newColor} onChange={(e) => setNewColor(e.target.value)} />
          <input
            className="manage-name"
            value={newName}
            placeholder={`New ${props.noun}…`}
            onChange={(e) => setNewName(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && add()}
          />
          <button className="primary" onClick={add}>
            Add
          </button>
        </div>
      </div>
    </div>
  );
}
