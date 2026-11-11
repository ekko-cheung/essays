CREATE TABLE notebook (
  id         TEXT PRIMARY KEY,
  parent_id  TEXT REFERENCES notebook(id) ON DELETE CASCADE,
  name       TEXT NOT NULL,
  sort_order INTEGER NOT NULL DEFAULT 0,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  deleted_at INTEGER
);
CREATE INDEX idx_notebook_parent ON notebook(parent_id);

CREATE TABLE status (
  id         TEXT PRIMARY KEY,
  name       TEXT NOT NULL UNIQUE,
  color      TEXT NOT NULL DEFAULT '#888888',
  sort_order INTEGER NOT NULL DEFAULT 0,
  is_default INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE note (
  id          TEXT PRIMARY KEY,
  notebook_id TEXT NOT NULL REFERENCES notebook(id) ON DELETE CASCADE,
  title       TEXT NOT NULL DEFAULT '',
  body_md     TEXT NOT NULL DEFAULT '',
  status_id   TEXT REFERENCES status(id) ON DELETE SET NULL,
  created_at  INTEGER NOT NULL,
  updated_at  INTEGER NOT NULL,
  deleted_at  INTEGER
);
CREATE INDEX idx_note_notebook ON note(notebook_id);
CREATE INDEX idx_note_status ON note(status_id);

CREATE TABLE tag (
  id    TEXT PRIMARY KEY,
  name  TEXT NOT NULL UNIQUE,
  color TEXT NOT NULL DEFAULT '#888888'
);

CREATE TABLE note_tag (
  note_id TEXT NOT NULL REFERENCES note(id) ON DELETE CASCADE,
  tag_id  TEXT NOT NULL REFERENCES tag(id)  ON DELETE CASCADE,
  PRIMARY KEY (note_id, tag_id)
);
CREATE INDEX idx_note_tag_tag ON note_tag(tag_id);

CREATE VIRTUAL TABLE note_fts USING fts5(title, body_md, content='note', content_rowid='rowid');
CREATE TRIGGER note_ai AFTER INSERT ON note BEGIN
  INSERT INTO note_fts(rowid, title, body_md) VALUES (new.rowid, new.title, new.body_md);
END;
CREATE TRIGGER note_ad AFTER DELETE ON note BEGIN
  INSERT INTO note_fts(note_fts, rowid, title, body_md) VALUES ('delete', old.rowid, old.title, old.body_md);
END;
CREATE TRIGGER note_au AFTER UPDATE ON note BEGIN
  INSERT INTO note_fts(note_fts, rowid, title, body_md) VALUES ('delete', old.rowid, old.title, old.body_md);
  INSERT INTO note_fts(rowid, title, body_md) VALUES (new.rowid, new.title, new.body_md);
END;

INSERT INTO status (id, name, color, sort_order, is_default) VALUES
  ('status-active',   'Active',   '#22c55e', 0, 1),
  ('status-on-hold',  'On Hold',  '#eab308', 1, 1),
  ('status-complete', 'Complete', '#3b82f6', 2, 1),
  ('status-dropped',  'Dropped',  '#9ca3af', 3, 1);
