//! Local half of cloud sync (the HTTP half lives in the app crate).
//!
//! Model: every synced row has an `mtime` and a local `dirty` flag (maintained by
//! triggers, see 0003_sync.sql). Sync uploads dirty rows and downloads rows the
//! remote has that this device hasn't seen. When both sides changed the same row,
//! the larger `mtime` wins (last write wins); deletions are rows with `deleted_at`.
//! A note's tag set travels inside the note row as a JSON array of tag ids.

use crate::{Db, Result};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NotebookRow {
    pub id: String,
    pub parent_id: Option<String>,
    pub name: String,
    pub sort_order: i64,
    pub created_at: i64,
    pub updated_at: i64,
    pub deleted_at: Option<i64>,
    pub mtime: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StatusRow {
    pub id: String,
    pub name: String,
    pub color: String,
    pub sort_order: i64,
    pub is_default: i64,
    pub deleted_at: Option<i64>,
    pub mtime: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TagRow {
    pub id: String,
    pub name: String,
    pub color: String,
    pub deleted_at: Option<i64>,
    pub mtime: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NoteRow {
    pub id: String,
    pub notebook_id: String,
    pub title: String,
    pub body_md: String,
    pub status_id: Option<String>,
    /// JSON array of tag ids.
    pub tag_ids: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub deleted_at: Option<i64>,
    pub mtime: i64,
}

#[derive(Debug, Clone, Default)]
pub struct Changes {
    pub notebooks: Vec<NotebookRow>,
    pub statuses: Vec<StatusRow>,
    pub tags: Vec<TagRow>,
    pub notes: Vec<NoteRow>,
}

impl Changes {
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub fn len(&self) -> usize {
        self.notebooks.len() + self.statuses.len() + self.tags.len() + self.notes.len()
    }
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyStats {
    pub applied: usize,
    pub changed_note_ids: Vec<String>,
}

/// The four synced tables, in the order they must be applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Table {
    Notebook,
    Status,
    Tag,
    Note,
}

impl Table {
    pub const ALL: [Table; 4] = [Table::Notebook, Table::Status, Table::Tag, Table::Note];

    pub fn remote(self) -> &'static str {
        match self {
            Table::Notebook => "sync_notebook",
            Table::Status => "sync_status",
            Table::Tag => "sync_tag",
            Table::Note => "sync_note",
        }
    }

    fn cursor_key(self) -> String {
        format!("cursor_{}", self.remote())
    }

    /// Column names (excluding the server-side `synced_at`).
    fn columns(self) -> &'static [&'static str] {
        match self {
            Table::Notebook => &[
                "id", "parent_id", "name", "sort_order", "created_at", "updated_at", "deleted_at", "mtime",
            ],
            Table::Status => &["id", "name", "color", "sort_order", "is_default", "deleted_at", "mtime"],
            Table::Tag => &["id", "name", "color", "deleted_at", "mtime"],
            Table::Note => &[
                "id", "notebook_id", "title", "body_md", "status_id", "tag_ids", "created_at",
                "updated_at", "deleted_at", "mtime",
            ],
        }
    }

    fn column_types(self) -> &'static str {
        match self {
            Table::Notebook => "id TEXT PRIMARY KEY, parent_id TEXT, name TEXT NOT NULL, sort_order INTEGER NOT NULL, created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL, deleted_at INTEGER, mtime INTEGER NOT NULL",
            Table::Status => "id TEXT PRIMARY KEY, name TEXT NOT NULL, color TEXT NOT NULL, sort_order INTEGER NOT NULL, is_default INTEGER NOT NULL, deleted_at INTEGER, mtime INTEGER NOT NULL",
            Table::Tag => "id TEXT PRIMARY KEY, name TEXT NOT NULL, color TEXT NOT NULL, deleted_at INTEGER, mtime INTEGER NOT NULL",
            Table::Note => "id TEXT PRIMARY KEY, notebook_id TEXT NOT NULL, title TEXT NOT NULL, body_md TEXT NOT NULL, status_id TEXT, tag_ids TEXT NOT NULL, created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL, deleted_at INTEGER, mtime INTEGER NOT NULL",
        }
    }
}

const SERVER_NOW_MS: &str = "CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER)";

/// Idempotent DDL for the remote (D1) database.
pub fn remote_schema() -> Vec<String> {
    let mut v = Vec::new();
    for t in Table::ALL {
        let name = t.remote();
        v.push(format!(
            "CREATE TABLE IF NOT EXISTS {name} ({}, synced_at INTEGER NOT NULL DEFAULT 0)",
            t.column_types()
        ));
        v.push(format!("CREATE INDEX IF NOT EXISTS idx_{name}_synced ON {name}(synced_at, id)"));
    }
    v
}

/// Upsert that only overwrites a remote row if the incoming one is strictly newer.
/// `synced_at` is stamped by the server clock so every device's pull cursor is
/// comparable regardless of device clock skew.
pub fn upsert_statement(table: Table, row: &Value) -> (String, Vec<Value>) {
    let cols = table.columns();
    let name = table.remote();
    let placeholders = vec!["?"; cols.len()].join(", ");
    let updates = cols[1..]
        .iter()
        .map(|c| format!("{c} = excluded.{c}"))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "INSERT INTO {name} ({cols}, synced_at) VALUES ({placeholders}, {SERVER_NOW_MS}) \
         ON CONFLICT(id) DO UPDATE SET {updates}, synced_at = {SERVER_NOW_MS} \
         WHERE excluded.mtime > {name}.mtime",
        cols = cols.join(", "),
    );
    let params = cols.iter().map(|c| row.get(*c).cloned().unwrap_or(Value::Null)).collect();
    (sql, params)
}

/// Page of rows with `synced_at >= cursor`, oldest first. Re-reading rows at the
/// cursor itself is harmless (apply is idempotent) and avoids missing same-ms writes.
pub fn pull_statement(table: Table, cursor: i64, after_id: &str, limit: usize) -> (String, Vec<Value>) {
    let sql = format!(
        "SELECT {cols}, synced_at FROM {name} \
         WHERE synced_at > ?1 OR (synced_at = ?1 AND id > ?2) ORDER BY synced_at, id LIMIT {limit}",
        cols = table.columns().join(", "),
        name = table.remote(),
    );
    (sql, vec![cursor.into(), after_id.into()])
}

impl Db {
    pub fn sync_state(&self, key: &str) -> Result<Option<String>> {
        Ok(self
            .conn
            .query_row("SELECT value FROM sync_state WHERE key = ?1", params![key], |r| r.get(0))
            .optional()?)
    }

    pub fn set_sync_state(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO sync_state (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn cursor(&self, table: Table) -> Result<i64> {
        Ok(self.sync_state(&table.cursor_key())?.and_then(|v| v.parse().ok()).unwrap_or(0))
    }

    pub fn set_cursor(&self, table: Table, value: i64) -> Result<()> {
        self.set_sync_state(&table.cursor_key(), &value.to_string())
    }

    /// Forget what was synced: everything is uploaded again and pulled from scratch.
    /// Used when the remote database changes.
    pub fn reset_sync(&self) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        tx.execute("DELETE FROM sync_state WHERE key LIKE 'cursor_%'", [])?;
        for t in ["notebook", "note", "status", "tag"] {
            tx.execute(&format!("UPDATE {t} SET dirty = 1 WHERE dirty = 0"), [])?;
        }
        tx.execute("UPDATE sync_tombstone SET dirty = 1", [])?;
        tx.commit()?;
        Ok(())
    }

    pub fn dirty_count(&self) -> Result<i64> {
        let mut n = 0;
        for t in ["notebook", "note", "status", "tag"] {
            n += self.conn.query_row(&format!("SELECT COUNT(*) FROM {t} WHERE dirty = 1"), [], |r| {
                r.get::<_, i64>(0)
            })?;
        }
        n += self
            .conn
            .query_row("SELECT COUNT(*) FROM sync_tombstone WHERE dirty = 1", [], |r| r.get::<_, i64>(0))?;
        Ok(n)
    }

    /// Up to `limit` locally-modified rows per table (tombstones included).
    pub fn pending_changes(&self, limit: usize) -> Result<Changes> {
        let notebooks = self
            .conn
            .prepare(
                "SELECT id, parent_id, name, sort_order, created_at, updated_at, deleted_at, mtime
                 FROM notebook WHERE dirty = 1 LIMIT ?1",
            )?
            .query_map(params![limit as i64], |r| {
                Ok(NotebookRow {
                    id: r.get(0)?,
                    parent_id: r.get(1)?,
                    name: r.get(2)?,
                    sort_order: r.get(3)?,
                    created_at: r.get(4)?,
                    updated_at: r.get(5)?,
                    deleted_at: r.get(6)?,
                    mtime: r.get(7)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        let mut statuses = self
            .conn
            .prepare(
                "SELECT id, name, color, sort_order, is_default, mtime FROM status
                 WHERE dirty = 1 LIMIT ?1",
            )?
            .query_map(params![limit as i64], |r| {
                Ok(StatusRow {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    color: r.get(2)?,
                    sort_order: r.get(3)?,
                    is_default: r.get(4)?,
                    deleted_at: None,
                    mtime: r.get(5)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let mut tags = self
            .conn
            .prepare("SELECT id, name, color, mtime FROM tag WHERE dirty = 1 LIMIT ?1")?
            .query_map(params![limit as i64], |r| {
                Ok(TagRow { id: r.get(0)?, name: r.get(1)?, color: r.get(2)?, deleted_at: None, mtime: r.get(3)? })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let mut tombstones = self.conn.prepare(
            "SELECT tbl, id, mtime FROM sync_tombstone WHERE dirty = 1 LIMIT ?1",
        )?;
        let tombs = tombstones
            .query_map(params![limit as i64], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        for (tbl, id, mtime) in tombs {
            if tbl == "status" {
                statuses.push(StatusRow {
                    id,
                    name: String::new(),
                    color: "#888888".into(),
                    sort_order: 0,
                    is_default: 0,
                    deleted_at: Some(mtime),
                    mtime,
                });
            } else {
                tags.push(TagRow { id, name: String::new(), color: "#888888".into(), deleted_at: Some(mtime), mtime });
            }
        }

        let mut notes = self
            .conn
            .prepare(
                "SELECT id, notebook_id, title, body_md, status_id, created_at, updated_at, deleted_at, mtime
                 FROM note WHERE dirty = 1 LIMIT ?1",
            )?
            .query_map(params![limit as i64], |r| {
                Ok(NoteRow {
                    id: r.get(0)?,
                    notebook_id: r.get(1)?,
                    title: r.get(2)?,
                    body_md: r.get(3)?,
                    status_id: r.get(4)?,
                    tag_ids: String::new(),
                    created_at: r.get(5)?,
                    updated_at: r.get(6)?,
                    deleted_at: r.get(7)?,
                    mtime: r.get(8)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        for n in &mut notes {
            let ids: Vec<String> = self
                .conn
                .prepare("SELECT tag_id FROM note_tag WHERE note_id = ?1 ORDER BY tag_id")?
                .query_map(params![n.id], |r| r.get(0))?
                .collect::<std::result::Result<_, _>>()?;
            n.tag_ids = serde_json::to_string(&ids).unwrap_or_else(|_| "[]".into());
        }
        Ok(Changes { notebooks, statuses, tags, notes })
    }

    /// Clear `dirty` for rows that were uploaded, unless they changed again meanwhile.
    pub fn mark_pushed(&self, c: &Changes) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        for r in &c.notebooks {
            tx.execute("UPDATE notebook SET dirty = 0 WHERE id = ?1 AND mtime = ?2", params![r.id, r.mtime])?;
        }
        for r in &c.statuses {
            if r.deleted_at.is_some() {
                tx.execute(
                    "UPDATE sync_tombstone SET dirty = 0 WHERE tbl = 'status' AND id = ?1 AND mtime = ?2",
                    params![r.id, r.mtime],
                )?;
            } else {
                tx.execute("UPDATE status SET dirty = 0 WHERE id = ?1 AND mtime = ?2", params![r.id, r.mtime])?;
            }
        }
        for r in &c.tags {
            if r.deleted_at.is_some() {
                tx.execute(
                    "UPDATE sync_tombstone SET dirty = 0 WHERE tbl = 'tag' AND id = ?1 AND mtime = ?2",
                    params![r.id, r.mtime],
                )?;
            } else {
                tx.execute("UPDATE tag SET dirty = 0 WHERE id = ?1 AND mtime = ?2", params![r.id, r.mtime])?;
            }
        }
        for r in &c.notes {
            tx.execute("UPDATE note SET dirty = 0 WHERE id = ?1 AND mtime = ?2", params![r.id, r.mtime])?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Merge rows from the remote. A row is applied only if it is newer than the
    /// local copy, so this is idempotent and never clobbers newer local edits.
    pub fn apply_changes(&self, c: &Changes) -> Result<ApplyStats> {
        let tx = self.conn.unchecked_transaction()?;
        // Rows may arrive before the rows they reference; check at commit instead.
        tx.execute_batch("PRAGMA defer_foreign_keys = ON")?;
        let mut stats = ApplyStats::default();

        for r in &c.notebooks {
            if r.mtime <= local_mtime(&tx, "notebook", &r.id)? {
                continue;
            }
            tx.execute(
                "INSERT INTO notebook (id, parent_id, name, sort_order, created_at, updated_at, deleted_at, mtime, dirty)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 0)
                 ON CONFLICT(id) DO UPDATE SET parent_id = excluded.parent_id, name = excluded.name,
                   sort_order = excluded.sort_order, updated_at = excluded.updated_at,
                   deleted_at = excluded.deleted_at, mtime = excluded.mtime, dirty = 0",
                params![r.id, r.parent_id, r.name, r.sort_order, r.created_at, r.updated_at, r.deleted_at, r.mtime],
            )?;
            stats.applied += 1;
        }

        for r in &c.statuses {
            if r.mtime <= local_mtime(&tx, "status", &r.id)?.max(tombstone_mtime(&tx, "status", &r.id)?) {
                continue;
            }
            if r.deleted_at.is_some() {
                tx.execute("DELETE FROM status WHERE id = ?1", params![r.id])?;
                set_tombstone(&tx, "status", &r.id, r.mtime)?;
            } else {
                if let Some(dup) = same_name(&tx, "status", &r.id, &r.name)? {
                    // Same-named status created independently on two devices: merge into the remote one.
                    tx.execute("UPDATE note SET status_id = ?1 WHERE status_id = ?2", params![r.id, dup])?;
                    drop_without_tombstone(&tx, "status", &dup)?;
                }
                tx.execute(
                    "INSERT INTO status (id, name, color, sort_order, is_default, mtime, dirty)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0)
                     ON CONFLICT(id) DO UPDATE SET name = excluded.name, color = excluded.color,
                       sort_order = excluded.sort_order, is_default = excluded.is_default,
                       mtime = excluded.mtime, dirty = 0",
                    params![r.id, r.name, r.color, r.sort_order, r.is_default, r.mtime],
                )?;
                tx.execute("DELETE FROM sync_tombstone WHERE tbl = 'status' AND id = ?1", params![r.id])?;
            }
            stats.applied += 1;
        }

        for r in &c.tags {
            if r.mtime <= local_mtime(&tx, "tag", &r.id)?.max(tombstone_mtime(&tx, "tag", &r.id)?) {
                continue;
            }
            if r.deleted_at.is_some() {
                tx.execute("DELETE FROM tag WHERE id = ?1", params![r.id])?;
                set_tombstone(&tx, "tag", &r.id, r.mtime)?;
            } else {
                if let Some(dup) = same_name(&tx, "tag", &r.id, &r.name)? {
                    tx.execute("UPDATE OR IGNORE note_tag SET tag_id = ?1 WHERE tag_id = ?2", params![r.id, dup])?;
                    drop_without_tombstone(&tx, "tag", &dup)?;
                }
                tx.execute(
                    "INSERT INTO tag (id, name, color, mtime, dirty) VALUES (?1, ?2, ?3, ?4, 0)
                     ON CONFLICT(id) DO UPDATE SET name = excluded.name, color = excluded.color,
                       mtime = excluded.mtime, dirty = 0",
                    params![r.id, r.name, r.color, r.mtime],
                )?;
                tx.execute("DELETE FROM sync_tombstone WHERE tbl = 'tag' AND id = ?1", params![r.id])?;
            }
            stats.applied += 1;
        }

        for r in &c.notes {
            if r.mtime <= local_mtime(&tx, "note", &r.id)? {
                continue;
            }
            let notebook_known: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM notebook WHERE id = ?1)",
                params![r.notebook_id],
                |x| x.get(0),
            )?;
            if !notebook_known {
                continue;
            }
            // A status deleted elsewhere must not leave a dangling reference.
            let status_id: Option<String> = match &r.status_id {
                Some(s) => tx
                    .query_row("SELECT id FROM status WHERE id = ?1", params![s], |x| x.get(0))
                    .optional()?,
                None => None,
            };
            tx.execute(
                "INSERT INTO note (id, notebook_id, title, body_md, status_id, created_at, updated_at, deleted_at, mtime, dirty)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 0)
                 ON CONFLICT(id) DO UPDATE SET notebook_id = excluded.notebook_id, title = excluded.title,
                   body_md = excluded.body_md, status_id = excluded.status_id,
                   updated_at = excluded.updated_at, deleted_at = excluded.deleted_at,
                   mtime = excluded.mtime, dirty = 0",
                params![r.id, r.notebook_id, r.title, r.body_md, status_id, r.created_at, r.updated_at, r.deleted_at, r.mtime],
            )?;

            let wanted: Vec<String> = serde_json::from_str(&r.tag_ids).unwrap_or_default();
            let mut want_ids = Vec::new();
            for t in wanted {
                let known: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM tag WHERE id = ?1)", params![t], |x| x.get(0))?;
                if known {
                    want_ids.push(t);
                }
            }
            want_ids.sort();
            want_ids.dedup();
            let have: Vec<String> = tx
                .prepare("SELECT tag_id FROM note_tag WHERE note_id = ?1 ORDER BY tag_id")?
                .query_map(params![r.id], |x| x.get(0))?
                .collect::<std::result::Result<_, _>>()?;
            if have != want_ids {
                tx.execute("DELETE FROM note_tag WHERE note_id = ?1", params![r.id])?;
                for t in &want_ids {
                    tx.execute("INSERT INTO note_tag (note_id, tag_id) VALUES (?1, ?2)", params![r.id, t])?;
                }
                // The tag edits above dirtied the note via triggers; this is not a local edit.
                tx.execute("UPDATE note SET mtime = ?2, dirty = 0 WHERE id = ?1", params![r.id, r.mtime])?;
            }
            stats.changed_note_ids.push(r.id.clone());
            stats.applied += 1;
        }

        tx.commit()?;
        Ok(stats)
    }
}

fn local_mtime(tx: &rusqlite::Connection, table: &str, id: &str) -> Result<i64> {
    Ok(tx
        .query_row(&format!("SELECT mtime FROM {table} WHERE id = ?1"), params![id], |r| r.get(0))
        .optional()?
        .unwrap_or(-1))
}

fn tombstone_mtime(tx: &rusqlite::Connection, tbl: &str, id: &str) -> Result<i64> {
    Ok(tx
        .query_row("SELECT mtime FROM sync_tombstone WHERE tbl = ?1 AND id = ?2", params![tbl, id], |r| r.get(0))
        .optional()?
        .unwrap_or(-1))
}

fn set_tombstone(tx: &rusqlite::Connection, tbl: &str, id: &str, mtime: i64) -> Result<()> {
    tx.execute(
        "INSERT OR REPLACE INTO sync_tombstone (tbl, id, mtime, dirty) VALUES (?1, ?2, ?3, 0)",
        params![tbl, id, mtime],
    )?;
    Ok(())
}

/// Another local row with the same (case-insensitive) name but a different id.
fn same_name(tx: &rusqlite::Connection, table: &str, id: &str, name: &str) -> Result<Option<String>> {
    Ok(tx
        .query_row(
            &format!("SELECT id FROM {table} WHERE lower(name) = lower(?1) AND id != ?2"),
            params![name, id],
            |r| r.get(0),
        )
        .optional()?)
}

/// Delete a merged-away duplicate locally without telling the remote it's gone.
fn drop_without_tombstone(tx: &rusqlite::Connection, table: &str, id: &str) -> Result<()> {
    tx.execute(&format!("DELETE FROM {table} WHERE id = ?1"), params![id])?;
    tx.execute("DELETE FROM sync_tombstone WHERE tbl = ?1 AND id = ?2", params![table, id])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two devices syncing through an in-memory "remote" that mimics the D1 rules.
    #[derive(Default)]
    struct Remote(Changes);

    impl Remote {
        fn push(&mut self, c: &Changes) {
            macro_rules! up {
                ($field:ident) => {
                    for r in &c.$field {
                        match self.0.$field.iter_mut().find(|x| x.id == r.id) {
                            Some(x) if r.mtime > x.mtime => *x = r.clone(),
                            Some(_) => {}
                            None => self.0.$field.push(r.clone()),
                        }
                    }
                };
            }
            up!(notebooks);
            up!(statuses);
            up!(tags);
            up!(notes);
        }
    }

    fn sync(db: &Db, remote: &mut Remote) {
        db.apply_changes(&remote.0).unwrap();
        let pending = db.pending_changes(10_000).unwrap();
        remote.push(&pending);
        db.mark_pushed(&pending).unwrap();
    }

    fn titles(db: &Db) -> Vec<String> {
        let mut v: Vec<String> = db
            .conn
            .prepare("SELECT title FROM note WHERE deleted_at IS NULL ORDER BY title")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<std::result::Result<_, _>>()
            .unwrap();
        v.sort();
        v
    }

    #[test]
    fn notes_and_tags_flow_between_devices_and_settle() {
        let (a, b) = (Db::open_in_memory().unwrap(), Db::open_in_memory().unwrap());
        let mut remote = Remote::default();
        let nb = a.create_notebook(None, "NB").unwrap();
        let note = a.create_note(&nb.id).unwrap();
        a.update_note(&note.id, "hello", "body").unwrap();
        let tag = a.create_tag("rust", "#112233").unwrap();
        a.set_note_tags(&note.id, &[tag.id.clone()]).unwrap();

        sync(&a, &mut remote);
        sync(&b, &mut remote);
        let got = b.get_note(&note.id).unwrap();
        assert_eq!((got.title.as_str(), got.body_md.as_str()), ("hello", "body"));
        assert_eq!(got.tag_ids, [tag.id.clone()]);

        // Nothing left to upload on either side after a round trip.
        assert_eq!(a.dirty_count().unwrap(), 0);
        assert_eq!(b.dirty_count().unwrap(), 0);
    }

    #[test]
    fn newer_edit_wins_and_deletions_propagate() {
        let (a, b) = (Db::open_in_memory().unwrap(), Db::open_in_memory().unwrap());
        let mut remote = Remote::default();
        let nb = a.create_notebook(None, "NB").unwrap();
        let n1 = a.create_note(&nb.id).unwrap();
        let n2 = a.create_note(&nb.id).unwrap();
        a.update_note(&n1.id, "one", "").unwrap();
        a.update_note(&n2.id, "two", "").unwrap();
        sync(&a, &mut remote);
        sync(&b, &mut remote);

        b.update_note(&n1.id, "one-edited", "").unwrap();
        b.delete_note(&n2.id).unwrap();
        sync(&b, &mut remote);
        sync(&a, &mut remote);
        assert_eq!(titles(&a), ["one-edited"]);
    }

    #[test]
    fn deleted_status_and_tag_propagate_and_clear_notes() {
        let (a, b) = (Db::open_in_memory().unwrap(), Db::open_in_memory().unwrap());
        let mut remote = Remote::default();
        let nb = a.create_notebook(None, "NB").unwrap();
        let note = a.create_note(&nb.id).unwrap();
        let tag = a.create_tag("t", "#000000").unwrap();
        a.set_note_tags(&note.id, &[tag.id.clone()]).unwrap();
        sync(&a, &mut remote);
        sync(&b, &mut remote);

        b.delete_tag(&tag.id).unwrap();
        b.delete_status("status-active").unwrap();
        sync(&b, &mut remote);
        sync(&a, &mut remote);
        let got = a.get_note(&note.id).unwrap();
        assert!(got.tag_ids.is_empty());
        assert_eq!(got.status_id, None);
        assert!(a.list_tags().unwrap().is_empty());
        assert_eq!(a.list_statuses().unwrap().len(), 3);
        // The deletion made on B was applied on A without bouncing back as a new change.
        sync(&a, &mut remote);
        assert_eq!(a.dirty_count().unwrap(), 0);
    }

    #[test]
    fn same_named_tags_created_on_two_devices_merge() {
        let (a, b) = (Db::open_in_memory().unwrap(), Db::open_in_memory().unwrap());
        let mut remote = Remote::default();
        let nb = a.create_notebook(None, "NB").unwrap();
        let note = a.create_note(&nb.id).unwrap();
        sync(&a, &mut remote);
        sync(&b, &mut remote);

        let ta = a.create_tag("idea", "#111111").unwrap();
        a.set_note_tags(&note.id, &[ta.id.clone()]).unwrap();
        let tb = b.create_tag("Idea", "#222222").unwrap();
        sync(&a, &mut remote);
        sync(&b, &mut remote);
        sync(&a, &mut remote);
        sync(&b, &mut remote);
        assert_eq!(a.list_tags().unwrap().len(), 1);
        assert_eq!(b.list_tags().unwrap().len(), 1);
        assert_ne!(ta.id, tb.id);
        assert_eq!(a.get_note(&note.id).unwrap().tag_ids.len(), 1);
        assert_eq!(b.get_note(&note.id).unwrap().tag_ids.len(), 1);
    }

    #[test]
    fn upsert_statement_binds_every_column() {
        let (sql, params) = upsert_statement(
            Table::Tag,
            &serde_json::json!({"id": "x", "name": "n", "color": "#000000", "deleted_at": null, "mtime": 5}),
        );
        assert_eq!(params.len(), 5);
        assert_eq!(sql.matches('?').count(), 5);
        assert!(sql.contains("WHERE excluded.mtime > sync_tag.mtime"));
    }
}
