import { invoke } from "@tauri-apps/api/core";

export interface Notebook {
  id: string;
  parentId: string | null;
  name: string;
  sortOrder: number;
}

export interface Note {
  id: string;
  notebookId: string;
  title: string;
  bodyMd: string;
  statusId: string | null;
  tagIds: string[];
  createdAt: number;
  updatedAt: number;
}

export interface NoteSummary {
  id: string;
  notebookId: string;
  title: string;
  snippet: string;
  statusId: string | null;
  tagIds: string[];
  updatedAt: number;
}

/** status: OR · tags: AND · search: every word · all groups ANDed. */
export interface NoteQuery {
  notebookId: string | null; // null = all notebooks
  includeDescendants: boolean;
  statusIds: string[];
  noStatus: boolean;
  tagIds: string[];
  search: string;
}

export interface Status {
  id: string;
  name: string;
  color: string;
  sortOrder: number;
  isDefault: boolean;
  noteCount: number;
}

export interface Tag {
  id: string;
  name: string;
  color: string;
  noteCount: number;
}

// Single data-access seam: swap `invoke` for `fetch` here when a web build exists.
export const api = {
  listNotebooks: () => invoke<Notebook[]>("list_notebooks"),
  createNotebook: (parentId: string | null, name: string) =>
    invoke<Notebook>("create_notebook", { parentId, name }),
  renameNotebook: (id: string, name: string) =>
    invoke<void>("rename_notebook", { id, name }),
  moveNotebook: (id: string, newParentId: string | null, index: number) =>
    invoke<void>("move_notebook", { id, newParentId, index }),
  deleteNotebook: (id: string) => invoke<void>("delete_notebook", { id }),
  queryNotes: (query: NoteQuery) => invoke<NoteSummary[]>("query_notes", { query }),
  getNote: (id: string) => invoke<Note>("get_note", { id }),
  createNote: (notebookId: string) =>
    invoke<Note>("create_note", { notebookId }),
  updateNote: (id: string, title: string, bodyMd: string) =>
    invoke<number>("update_note", { id, title, bodyMd }),
  deleteNote: (id: string) => invoke<void>("delete_note", { id }),

  listStatuses: () => invoke<Status[]>("list_statuses"),
  createStatus: (name: string, color: string) =>
    invoke<Status>("create_status", { name, color }),
  updateStatus: (id: string, name: string, color: string) =>
    invoke<void>("update_status", { id, name, color }),
  deleteStatus: (id: string) => invoke<void>("delete_status", { id }),

  listTags: () => invoke<Tag[]>("list_tags"),
  createTag: (name: string, color?: string) =>
    invoke<Tag>("create_tag", { name, color: color ?? null }),
  updateTag: (id: string, name: string, color: string) =>
    invoke<void>("update_tag", { id, name, color }),
  deleteTag: (id: string) => invoke<void>("delete_tag", { id }),

  setNoteStatus: (noteId: string, statusId: string | null) =>
    invoke<void>("set_note_status", { noteId, statusId }),
  setNoteTags: (noteId: string, tagIds: string[]) =>
    invoke<void>("set_note_tags", { noteId, tagIds }),
};
