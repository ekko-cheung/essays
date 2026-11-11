import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  api,
  type Note,
  type Notebook,
  type Settings,
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
import { HelpDialog } from "./HelpDialog";
import { SettingsDialog } from "./SettingsDialog";
import { Icon, type IconName } from "./Icon";
import { EVT_SAVE_NOW, handleShortcut, type ShortcutHandlers } from "./shortcuts";
import { getThemePref, nextThemePref, setThemePref, type ThemePref } from "./theme";
import { ALL_NOTES, buildTree, computeMove, subtreeIds, type DropPosition } from "./tree";
import { useConfirm } from "./confirm";
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
  const confirm = useConfirm();
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
  const [helpOpen, setHelpOpen] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [settings, setSettings] = useState<Settings | null>(null);
  const [syncing, setSyncing] = useState(false);
  // Bumped when a sync rewrites the open note, to remount the editor on the new text.
  const [noteRev, setNoteRev] = useState(0);
  const [themePref, setThemePrefState] = useState<ThemePref>(getThemePref);
  const searchRef = useRef<HTMLInputElement>(null);

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

  useEffect(() => {
    api.getSettings().then(setSettings).catch((e) => setError(String(e)));
  }, []);

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

  const createNote = () => {
    if (isAll) {
      setError("Select a notebook first to create a note in it.");
      return;
    }
    void run(async () => {
      const n = await api.createNote(notebookId);
      setNoteId(n.id);
      // A fresh note is Active/untagged/empty: clear any narrowing so it shows up.
      setSearch("");
      setDebouncedSearch("");
      changeFilters(emptyFilters(filters.includeSub));
    }, refreshNotes);
  };

  // One window listener for the whole app; the ref keeps it pointing at fresh closures.
  const shortcutHandlers = useRef<ShortcutHandlers>(null!);
  shortcutHandlers.current = {
    newNote: createNote,
    focusSearch: () => {
      searchRef.current?.focus();
      searchRef.current?.select();
    },
    toggleHelp: () => setHelpOpen((v) => !v),
    openSettings: () => setSettingsOpen(true),
  };
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => void handleShortcut(e, shortcutHandlers.current);
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  const cycleTheme = () => {
    const next = nextThemePref(themePref);
    setThemePref(next);
    setThemePrefState(next);
  };

  // Throws on failure so the settings dialog can show the reason; background
  // callers go through `syncQuietly`.
  const syncing_ = useRef(false);
  const noteIdRef = useRef(noteId);
  noteIdRef.current = noteId;
  const notebookIdRef = useRef(notebookId);
  notebookIdRef.current = notebookId;
  const syncNow = useCallback(async () => {
    if (syncing_.current) return;
    syncing_.current = true;
    setSyncing(true);
    try {
      // Push any text still sitting in the editor's debounce window first.
      window.dispatchEvent(new Event(EVT_SAVE_NOW));
      await new Promise((r) => setTimeout(r, 50));
      const report = await api.syncNow();
      if (report.pulled === 0) return;
      const nbs = await api.listNotebooks();
      setNotebooks(nbs);
      if (notebookIdRef.current !== ALL_NOTES && !nbs.some((n) => n.id === notebookIdRef.current)) {
        setNotebookId(ALL_NOTES);
        setNoteId(null);
      }
      await Promise.all([refreshStatuses(), refreshTags(), refreshNotes()]);
      const open = noteIdRef.current;
      if (open && report.changedNoteIds.includes(open)) {
        try {
          setNote(await api.getNote(open));
          setNoteRev((r) => r + 1);
        } catch {
          setNoteId(null); // deleted on another device
        }
      }
    } finally {
      syncing_.current = false;
      setSyncing(false);
    }
  }, [refreshNotes, refreshStatuses, refreshTags]);

  const syncQuietly = useCallback(
    () => syncNow().catch((e) => setError(`Sync failed: ${String(e)}`)),
    [syncNow],
  );

  // Sync on launch and then on a timer while enabled.
  const autoSync = settings?.syncEnabled && settings.autoSync && settings.hasToken;
  const intervalMin = settings?.intervalMinutes ?? 5;
  useEffect(() => {
    if (!ready || !autoSync) return;
    void syncQuietly();
    const t = window.setInterval(() => void syncQuietly(), intervalMin * 60_000);
    return () => window.clearInterval(t);
  }, [ready, autoSync, intervalMin, syncQuietly]);

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
          onDelete={async (id, name) => {
            const doomed = subtreeIds(notebooks, id);
            const subs = doomed.size - 1;
            const ok = await confirm({
              title: `Delete notebook "${name}"?`,
              message: `${
                subs > 0 ? `Its ${subs} sub-notebook${subs > 1 ? "s" : ""} and all` : "All"
              } notes inside will be deleted too. This can't be undone.`,
              confirmLabel: "Delete",
              danger: true,
            });
            if (!ok) return;
            await run(async () => {
              await api.deleteNotebook(id);
              // Clear selection if it was inside the deleted subtree (or the notebook itself).
              if (doomed.has(notebookId)) {
                setNotebookId(ALL_NOTES);
                setNoteId(null);
              }
            }, refreshNotebooks);
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
        <div className="sidebar-foot">
          <button onClick={cycleTheme} title="Switch theme: system / light / dark">
            <Icon name={{ system: "monitor", light: "sun", dark: "moon" }[themePref] as IconName} size={15} />
            {{ system: "System", light: "Light", dark: "Dark" }[themePref]}
          </button>
          {settings?.syncEnabled && (
            <button onClick={() => void syncQuietly()} title="Sync now" aria-label="Sync now" disabled={syncing}>
              <Icon name="refresh" size={15} className={syncing ? "spin" : ""} />
            </button>
          )}
          <button onClick={() => setSettingsOpen(true)} title="Settings" aria-label="Settings">
            <Icon name="settings" size={15} />
          </button>
          <button onClick={() => setHelpOpen(true)} title="Keyboard shortcuts" aria-label="Keyboard shortcuts">
            <Icon name="help" size={15} />
          </button>
        </div>
      </aside>

      <NoteList
        title={isAll ? "All Notes" : (notebook?.name ?? "")}
        canCreate={!isAll}
        notebookNameById={isAll || filters.includeSub ? notebookNameById : null}
        narrowed={narrowed}
        filterBar={
          <SearchBar
            inputRef={searchRef}
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
        onCreate={createNote}
        onDelete={async (id, title) => {
          const ok = await confirm({
            title: `Delete note "${title}"?`,
            message: "This can't be undone.",
            confirmLabel: "Delete",
            danger: true,
          });
          if (!ok) return;
          await run(async () => {
            await api.deleteNote(id);
            if (noteId === id) setNoteId(null);
          }, refreshNotes);
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
            key={`${note.id}:${noteRev}`}
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
          <div className="empty-state placeholder">
            <Icon name="note" size={40} />
            <div>Select {isAll ? "a note" : "or create a note"}</div>
            {!isAll && (
              <button className="primary" onClick={createNote}>
                New note
              </button>
            )}
          </div>
        )}
      </main>

      {helpOpen && <HelpDialog onClose={() => setHelpOpen(false)} />}
      {settingsOpen && settings && (
        <SettingsDialog
          settings={settings}
          syncing={syncing}
          onSaved={setSettings}
          onSyncNow={syncNow}
          onClose={() => setSettingsOpen(false)}
        />
      )}
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
            const ok = await confirm({
              title: `Delete status "${s.name}"?`,
              message: s.noteCount
                ? `${s.noteCount} note${s.noteCount > 1 ? "s" : ""} using it will have no status.`
                : "No notes are using this status.",
              confirmLabel: "Delete",
              danger: true,
            });
            if (!ok) return;
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
            const ok = await confirm({
              title: `Delete tag "${t.name}"?`,
              message: t.noteCount
                ? `It will be removed from ${t.noteCount} note${t.noteCount > 1 ? "s" : ""}.`
                : "No notes are using this tag.",
              confirmLabel: "Delete",
              danger: true,
            });
            if (!ok) return;
            await api.deleteTag(t.id);
            await Promise.all([refreshTags(), refreshNotes()]);
          }}
          onClose={() => setDialog(null)}
        />
      )}
    </div>
  );
}
