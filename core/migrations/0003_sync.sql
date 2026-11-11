-- Sync bookkeeping. Every synced row carries:
--   mtime : last-modified time (ms) used for last-write-wins between devices
--   dirty : 1 while the row has local changes not yet uploaded
-- Triggers maintain both for any local write, so no code path can forget to.
-- The sync applier writes an explicit mtime (and dirty = 0), which the triggers
-- recognise and leave alone.

ALTER TABLE notebook ADD COLUMN mtime INTEGER NOT NULL DEFAULT 0;
ALTER TABLE notebook ADD COLUMN dirty INTEGER NOT NULL DEFAULT 1;
ALTER TABLE note ADD COLUMN mtime INTEGER NOT NULL DEFAULT 0;
ALTER TABLE note ADD COLUMN dirty INTEGER NOT NULL DEFAULT 1;
ALTER TABLE status ADD COLUMN mtime INTEGER NOT NULL DEFAULT 0;
ALTER TABLE status ADD COLUMN dirty INTEGER NOT NULL DEFAULT 1;
ALTER TABLE tag ADD COLUMN mtime INTEGER NOT NULL DEFAULT 0;
ALTER TABLE tag ADD COLUMN dirty INTEGER NOT NULL DEFAULT 1;

UPDATE notebook SET mtime = max(updated_at, 1);
UPDATE note     SET mtime = max(updated_at, 1);
-- Built-in statuses get mtime 1 so any real edit made on another device wins.
UPDATE status   SET mtime = CASE WHEN is_default = 1 THEN 1 ELSE CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER) END;
UPDATE tag      SET mtime = CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER);

CREATE TRIGGER notebook_sync_ai AFTER INSERT ON notebook WHEN new.mtime = 0 BEGIN
  UPDATE notebook SET mtime = CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER) WHERE id = new.id;
END;
CREATE TRIGGER notebook_sync_au AFTER UPDATE ON notebook WHEN new.mtime = old.mtime AND new.dirty = old.dirty BEGIN
  UPDATE notebook SET mtime = max(CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER), old.mtime + 1), dirty = 1 WHERE id = new.id;
END;

CREATE TRIGGER note_sync_ai AFTER INSERT ON note WHEN new.mtime = 0 BEGIN
  UPDATE note SET mtime = CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER) WHERE id = new.id;
END;
CREATE TRIGGER note_sync_au AFTER UPDATE ON note WHEN new.mtime = old.mtime AND new.dirty = old.dirty BEGIN
  UPDATE note SET mtime = max(CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER), old.mtime + 1), dirty = 1 WHERE id = new.id;
END;

CREATE TRIGGER status_sync_ai AFTER INSERT ON status WHEN new.mtime = 0 BEGIN
  UPDATE status SET mtime = CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER) WHERE id = new.id;
END;
CREATE TRIGGER status_sync_au AFTER UPDATE ON status WHEN new.mtime = old.mtime AND new.dirty = old.dirty BEGIN
  UPDATE status SET mtime = max(CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER), old.mtime + 1), dirty = 1 WHERE id = new.id;
END;

CREATE TRIGGER tag_sync_ai AFTER INSERT ON tag WHEN new.mtime = 0 BEGIN
  UPDATE tag SET mtime = CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER) WHERE id = new.id;
END;
CREATE TRIGGER tag_sync_au AFTER UPDATE ON tag WHEN new.mtime = old.mtime AND new.dirty = old.dirty BEGIN
  UPDATE tag SET mtime = max(CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER), old.mtime + 1), dirty = 1 WHERE id = new.id;
END;

-- status and tag are hard-deleted locally, so deletions are remembered here.
CREATE TABLE sync_tombstone (
  tbl   TEXT NOT NULL,
  id    TEXT NOT NULL,
  mtime INTEGER NOT NULL,
  dirty INTEGER NOT NULL DEFAULT 1,
  PRIMARY KEY (tbl, id)
);
CREATE TRIGGER status_sync_ad AFTER DELETE ON status BEGIN
  INSERT OR REPLACE INTO sync_tombstone (tbl, id, mtime, dirty)
  VALUES ('status', old.id, max(CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER), old.mtime + 1), 1);
END;
CREATE TRIGGER tag_sync_ad AFTER DELETE ON tag BEGIN
  INSERT OR REPLACE INTO sync_tombstone (tbl, id, mtime, dirty)
  VALUES ('tag', old.id, max(CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER), old.mtime + 1), 1);
END;

-- A note's tag set is synced as part of the note, so tag changes dirty the note.
CREATE TRIGGER note_tag_sync_ai AFTER INSERT ON note_tag BEGIN
  UPDATE note SET mtime = max(CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER), mtime + 1), dirty = 1 WHERE id = new.note_id;
END;
CREATE TRIGGER note_tag_sync_ad AFTER DELETE ON note_tag BEGIN
  UPDATE note SET mtime = max(CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER), mtime + 1), dirty = 1 WHERE id = old.note_id;
END;

CREATE TABLE sync_state (key TEXT PRIMARY KEY, value TEXT NOT NULL);
