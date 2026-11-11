import { useCallback, useEffect, useRef, useState } from "react";
import { api, type Note, type Status, type Tag } from "./api";
import { MarkdownEditor } from "./MarkdownEditor";
import { MarkdownPreview } from "./MarkdownPreview";
import { NoteMeta } from "./NoteMeta";

type ViewMode = "edit" | "split" | "preview";
type SaveState = "saved" | "dirty" | "saving" | "error";

const SAVE_DELAY_MS = 600;

interface Props {
  note: Note;
  statuses: Status[];
  tags: Tag[];
  onSaved: () => void;
  /** Status/tags changed: lists and counts elsewhere need a refresh. */
  onMetaChanged: () => void;
  onCreateTag: (name: string) => Promise<Tag>;
  onError: (msg: string) => void;
}

/** Mounted once per note (parent uses key={note.id}); owns the draft and autosaves it. */
export function NoteEditor({
  note,
  statuses,
  tags,
  onSaved,
  onMetaChanged,
  onCreateTag,
  onError,
}: Props) {
  const [title, setTitle] = useState(note.title);
  const [body, setBody] = useState(note.bodyMd);
  const [statusId, setStatusId] = useState(note.statusId);
  const [tagIds, setTagIds] = useState(note.tagIds);
  const [mode, setMode] = useState<ViewMode>(
    () => (localStorage.getItem("viewMode") as ViewMode) || "split",
  );
  const [saveState, setSaveState] = useState<SaveState>("saved");

  const latest = useRef({ title: note.title, body: note.bodyMd });
  const dirty = useRef(false);
  const timer = useRef<number | undefined>(undefined);
  const callbacks = useRef({ onSaved, onError });
  callbacks.current = { onSaved, onError };

  const flush = useCallback(async () => {
    window.clearTimeout(timer.current);
    if (!dirty.current) return;
    dirty.current = false;
    setSaveState("saving");
    const { title, body } = latest.current;
    try {
      await api.updateNote(note.id, title, body);
      // If more edits arrived while saving, a new timer is already pending.
      if (!dirty.current) setSaveState("saved");
      callbacks.current.onSaved();
    } catch (e) {
      dirty.current = true;
      setSaveState("error");
      callbacks.current.onError(String(e));
    }
  }, [note.id]);

  const schedule = useCallback(() => {
    dirty.current = true;
    setSaveState("dirty");
    window.clearTimeout(timer.current);
    timer.current = window.setTimeout(flush, SAVE_DELAY_MS);
  }, [flush]);

  // Save pending edits when switching notes / closing the editor.
  useEffect(() => () => void flush(), [flush]);

  useEffect(() => {
    const onHide = () => void flush();
    window.addEventListener("beforeunload", onHide);
    window.addEventListener("blur", onHide);
    return () => {
      window.removeEventListener("beforeunload", onHide);
      window.removeEventListener("blur", onHide);
    };
  }, [flush]);

  // Status/tags are saved immediately through their own commands, so they never
  // race with (or get overwritten by) the debounced title/body save.
  const changeStatus = (id: string | null) => {
    const prev = statusId;
    setStatusId(id);
    api
      .setNoteStatus(note.id, id)
      .then(onMetaChanged)
      .catch((e) => {
        setStatusId(prev);
        onError(String(e));
      });
  };

  const changeTags = (ids: string[]) => {
    const prev = tagIds;
    setTagIds(ids);
    api
      .setNoteTags(note.id, ids)
      .then(onMetaChanged)
      .catch((e) => {
        setTagIds(prev);
        onError(String(e));
      });
  };

  const setViewMode = (m: ViewMode) => {
    setMode(m);
    try {
      localStorage.setItem("viewMode", m);
    } catch {
      /* storage unavailable: just don't remember */
    }
  };

  return (
    <div className="note-editor">
      <div className="note-head">
        <input
          className="title-input"
          value={title}
          placeholder="Untitled"
          onChange={(e) => {
            setTitle(e.target.value);
            latest.current.title = e.target.value;
            schedule();
          }}
        />
        <span className={`save-state ${saveState}`}>
          {{ saved: "Saved", dirty: "Editing…", saving: "Saving…", error: "Save failed" }[saveState]}
        </span>
        <div className="seg">
          {(["edit", "split", "preview"] as const).map((m) => (
            <button key={m} className={mode === m ? "on" : ""} onClick={() => setViewMode(m)}>
              {m}
            </button>
          ))}
        </div>
      </div>
      <NoteMeta
        statuses={statuses}
        tags={tags}
        statusId={statusId}
        tagIds={tagIds}
        onStatusChange={changeStatus}
        onTagsChange={changeTags}
        onCreateTag={onCreateTag}
      />
      <div className={`panes ${mode}`}>
        {mode !== "preview" && (
          <div className="pane editor-pane">
            <MarkdownEditor
              initialValue={note.bodyMd}
              onChange={(v) => {
                setBody(v);
                latest.current.body = v;
                schedule();
              }}
            />
          </div>
        )}
        {mode !== "edit" && (
          <div className="pane preview-pane">
            <MarkdownPreview source={body} />
          </div>
        )}
      </div>
    </div>
  );
}
