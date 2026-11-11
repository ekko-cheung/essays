import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  api,
  type Note,
  type Notebook,
  type NoteQuery,
  type NoteSummary,
  type Status,
  type Tag,
} from "./api";
import { NotebookTree } from "./NotebookTree";
import { NoteList } from "./NoteList";
import { NoteEditor } from "./NoteEditor";
import { ManageDialog } from "./ManageDialog";
import { SearchBar } from "./SearchBar";
import { SidebarFilters } from "./SidebarFilters";
import { activeCount, emptyFilters, type Filters } from "./filters";
import { ALL_NOTES, buildTree, computeMove, type DropPosition } from "./tree";
import "./App.css";

const SEARCH_DEBOUNCE_MS = 250;

function loadIncludeSub(): boolean {
  try {
    return localStorage.getItem("includeSub") !== "0";
  } catch {
    return true;
  }
}

export default function App() {
  const [notebooks, setNotebooks] = useState<Notebook[]>([]);
  const [notebookId, setNotebookId] = useState<string>(ALL_NOTES);
  const [filters, setFilters] = useState<Filters>(() => emptyFilters(loadIncludeSub()));
  const [search, setSearch] = useState("");
  const [debouncedSearch, setDebouncedSearch] = useState("");
  const [notes, setNotes] = useState<NoteSummary[]>([]);
  const [noteId, setNoteId] = useState<string | null>(null);
  const [note, setNote] = useState<Note | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [statuses, setStatuses] = useState<Status[]>([]);
  const [tags, setTags] = useState<Tag[]>([]);
  const [dialog, setDialog] = useState<"status" | "tag" | null>(null);

  const refreshStatuses = useCallback(async () => setStatuses(await api.listStatuses()), []);
  const refreshTags = useCallback(async () => setTags(await api.listTags()), []);

  const refreshNotebooks = useCallback(async () => {
    setNotebooks(await api.listNotebooks());
  }, []);

  // Show the splash/spinner until the first data is in, so the UI never flashes empty.
  const [ready, setReady] = useState(false);
  useEffect(() => {
    Promise.all([refreshNotebooks(), refreshStatuses(), refreshTags()])
      .catch((e) => setError(String(e)))
      .finally(() => setReady(true));
  }, [refreshNotebooks, refreshStatuses, refreshTags]);

  const isAll = notebookId === ALL_NOTES;

  useEffect(() => {
    const t = window.setTimeout(() => setDebouncedSearch(search), SEARCH_DEBOUNCE_MS);
    return () => window.clearTimeout(t);
  }, [search]);

  const query = useMemo<NoteQuery>(
    () => ({
      notebookId: isAll ? null : notebookId,
      includeDescendants: filters.includeSub,
      statusIds: filters.statusIds,
      noStatus: filters.noStatus,
      tagIds: filters.tagIds,
      search: debouncedSearch,
    }),
    [isAll, notebookId, filters, debouncedSearch],
  );

  // Responses can arrive out of order while typing; only the newest request may win.
  const querySeq = useRef(0);
  const refreshNotes = useCallback(async () => {
    const seq = ++querySeq.current;
    const result = await api.queryNotes(query);
    if (seq === querySeq.current) setNotes(result);
  }, [query]);

  const changeFilters = (f: Filters) => {
    setFilters(f);
    try {
      localStorage.setItem("includeSub", f.includeSub ? "1" : "0");
    } catch {
      /* storage unavailable: just don't remember */
    }
  };

  // Drop filter ids whose status/tag was deleted so they can't silently empty the list.
  useEffect(() => {
    setFilters((f) => {
      const sIds = f.statusIds.filter((id) => statuses.some((s) => s.id === id));
      const tIds = f.tagIds.filter((id) => tags.some((t) => t.id === id));
      if (sIds.length === f.statusIds.length && tIds.length === f.tagIds.length) return f;
      return { ...f, statusIds: sIds, tagIds: tIds };
    });
  }, [statuses, tags]);

  const [notesLoaded, setNotesLoaded] = useState(false);
  useEffect(() => {
    refreshNotes()
      .catch((e) => setError(String(e)))
      .finally(() => setNotesLoaded(true));
  }, [refreshNotes]);

  // Load the full note when the selection changes. `cancelled` guards against
  // a slow response for a previous selection overwriting a newer one.
  useEffect(() => {
    if (!noteId) {
      setNote(null);
      return;
    }
    let cancelled = false;
    api
      .getNote(noteId)
      .then((n) => !cancelled && setNote(n))
      .catch((e) => !cancelled && setError(String(e)));
    return () => {
      cancelled = true;
    };
  }, [noteId]);

  // Run a mutation, then reload; surface backend errors instead of swallowing them.
  const run = useCallback(async (fn: () => Promise<unknown>, after: () => Promise<unknown>) => {
    try {
      setError(null);
      await fn();
      await after();
    } catch (e) {
      setError(String(e));
    }
  }, []);

  const tree = useMemo(() => buildTree(notebooks), [notebooks]);
  const notebook = notebooks.find((n) => n.id === notebookId) ?? null;
  const notebookNameById = useMemo(() => new Map(notebooks.map((n) => [n.id, n.name])), [notebooks]);
  const narrowed = activeCount(filters) > 0 || debouncedSearch.trim() !== "";

  const selectNotebook = (id: string) => {
    if (id === notebookId) return;
    setNotebookId(id);
    setNoteId(null);
  };

  const onDrop = (dragId: string, targetId: string, pos: DropPosition) => {
    const move = computeMove(notebooks, dragId, targetId, pos);
    if (move) run(() => api.moveNotebook(dragId, move.newParentId, move.index), refreshNotebooks);
  };

  // Same markup/classes as the static splash in index.html (styled there), so the
  // handoff from splash to loading to app has no visual jump.
  if (!ready || !notesLoaded) {
    return (
      <div className="splash">
        <div className="spinner" />
      </div>
    );
  }

  return (
    <div className="app">
      <aside className="sidebar">
        <NotebookTree
          nodes={tree}
          selectedId={notebookId}
          onSelect={selectNotebook}
          onCreate={(parentId, name) =>
            run(async () => {
              const nb = await api.createNotebook(parentId, name);
              setNotebookId(nb.id);
              setNoteId(null);
            }, refreshNotebooks)
          }
          onRename={(id, name) => run(() => api.renameNotebook(id, name), refreshNotebooks)}
          onDelete={(id, name) => {
            if (window.confirm(`Delete "${name}" and everything inside it?`)) {
              run(async () => {
                await api.deleteNotebook(id);
                // Clear selection if it was inside the deleted subtree (or the notebook itself).
                const doomed = new Set<string>([id]);
                let grew = true;
                while (grew) {
                  grew = false;
                  for (const n of notebooks) {
                    if (n.parentId && doomed.has(n.parentId) && !doomed.has(n.id)) {
                      doomed.add(n.id);
                      grew = true;
                    }
                  }
                }
                if (doomed.has(notebookId)) {
                  setNotebookId(ALL_NOTES);
                  setNoteId(null);
                }
              }, refreshNotebooks);
            }
          }}
          onDrop={onDrop}
        />
        <SidebarFilters
          statuses={statuses}
          tags={tags}
          filters={filters}
          onFilters={changeFilters}
          onManage={setDialog}
        />
      </aside>

      <NoteList
        title={isAll ? "All Notes" : (notebook?.name ?? "")}
        canCreate={!isAll}
        notebookNameById={isAll || filters.includeSub ? notebookNameById : null}
        narrowed={narrowed}
        filterBar={
          <SearchBar
            search={search}
            onSearch={setSearch}
            canIncludeSub={!isAll}
            includeSub={filters.includeSub}
            onIncludeSub={(v) => changeFilters({ ...filters, includeSub: v })}
          />
        }
        notes={notes}
        statuses={statuses}
        tags={tags}
        selectedId={noteId}
        onSelect={setNoteId}
        onCreate={() =>
          run(async () => {
            const n = await api.createNote(notebookId);
            setNoteId(n.id);
            // A fresh note is Active/untagged/empty: clear any narrowing so it shows up.
            setSearch("");
            setDebouncedSearch("");
            changeFilters(emptyFilters(filters.includeSub));
          }, refreshNotes)
        }
        onDelete={(id, title) => {
          if (window.confirm(`Delete note "${title}"?`)) {
            run(async () => {
              await api.deleteNote(id);
              if (noteId === id) setNoteId(null);
            }, refreshNotes);
          }
        }}
      />

      <main className="main">
        {error && (
          <div className="error" onClick={() => setError(null)}>
            {error}
          </div>
        )}
        {note ? (
          <NoteEditor
            key={note.id}
            note={note}
            statuses={statuses}
            tags={tags}
            onSaved={() => void refreshNotes().catch((e) => setError(String(e)))}
            onMetaChanged={() =>
              void Promise.all([refreshNotes(), refreshStatuses(), refreshTags()]).catch((e) =>
                setError(String(e)),
              )
            }
            onCreateTag={async (name) => {
              const t = await api.createTag(name);
              await refreshTags();
              return t;
            }}
            onError={setError}
          />
        ) : (
          <p className="muted placeholder">
            Select {isAll ? "a note" : "or create a note"}
          </p>
        )}
      </main>

      {dialog === "status" && (
        <ManageDialog
          title="Statuses"
          noun="status"
          items={statuses}
          defaultColor="#8b5cf6"
          onCreate={async (name, color) => {
            await api.createStatus(name, color);
            await refreshStatuses();
          }}
          onUpdate={async (id, name, color) => {
            await api.updateStatus(id, name, color);
            await refreshStatuses();
          }}
          onDelete={async (s) => {
            const used = s.noteCount ? ` ${s.noteCount} note(s) will have no status.` : "";
            if (!window.confirm(`Delete status "${s.name}"?${used}`)) return;
            await api.deleteStatus(s.id);
            await Promise.all([refreshStatuses(), refreshNotes()]);
          }}
          onClose={() => setDialog(null)}
        />
      )}
      {dialog === "tag" && (
        <ManageDialog
          title="Tags"
          noun="tag"
          items={tags}
          defaultColor="#64748b"
          onCreate={async (name, color) => {
            await api.createTag(name, color);
            await refreshTags();
          }}
          onUpdate={async (id, name, color) => {
            await api.updateTag(id, name, color);
            await refreshTags();
          }}
          onDelete={async (t) => {
            const used = t.noteCount ? ` It will be removed from ${t.noteCount} note(s).` : "";
            if (!window.confirm(`Delete tag "${t.name}"?${used}`)) return;
            await api.deleteTag(t.id);
            await Promise.all([refreshTags(), refreshNotes()]);
          }}
          onClose={() => setDialog(null)}
        />
      )}
    </div>
  );
}
