use crate::query::NoteQuery;
use crate::{now_ms, Db, Error, Result};
use rusqlite::{params, OptionalExtension};
use serde::Serialize;

/// Default status assigned to new notes (seeded in 0001_init.sql).
pub const DEFAULT_STATUS_ID: &str = "status-active";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Note {
    pub id: String,
    pub notebook_id: String,
    pub title: String,
    pub body_md: String,
    pub status_id: Option<String>,
    pub tag_ids: Vec<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Lightweight row for list views: no full body, just a short preview.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct NoteSummary {
    pub id: String,
    pub notebook_id: String,
    pub title: String,
    pub snippet: String,
    pub status_id: Option<String>,
    pub tag_ids: Vec<String>,
    pub updated_at: i64,
}

impl Db {
    /// Live notes directly inside `notebook_id`, most recently updated first.
    pub fn list_notes(&self, notebook_id: &str) -> Result<Vec<NoteSummary>> {
        self.query_notes(&NoteQuery {
            notebook_id: Some(notebook_id.to_string()),
            ..Default::default()
        })
    }

    fn tag_ids_of(&self, note_id: &str) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT nt.tag_id FROM note_tag nt JOIN tag t ON t.id = nt.tag_id
             WHERE nt.note_id = ?1 ORDER BY lower(t.name)",
        )?;
        let ids = stmt
            .query_map(params![note_id], |r| r.get(0))?
            .collect::<std::result::Result<_, _>>()?;
        Ok(ids)
    }

    pub fn get_note(&self, id: &str) -> Result<Note> {
        let mut note = self
            .conn
            .query_row(
                "SELECT id, notebook_id, title, body_md, status_id, created_at, updated_at
                 FROM note WHERE id = ?1 AND deleted_at IS NULL",
                params![id],
                |r| {
                    Ok(Note {
                        id: r.get(0)?,
                        notebook_id: r.get(1)?,
                        title: r.get(2)?,
                        body_md: r.get(3)?,
                        status_id: r.get(4)?,
                        tag_ids: Vec::new(),
                        created_at: r.get(5)?,
                        updated_at: r.get(6)?,
                    })
                },
            )
            .optional()?
            .ok_or_else(|| Error::NotFound(id.into()))?;
        note.tag_ids = self.tag_ids_of(id)?;
        Ok(note)
    }

    pub fn create_note(&self, notebook_id: &str) -> Result<Note> {
        let live: Option<i64> = self
            .conn
            .query_row(
                "SELECT 1 FROM notebook WHERE id = ?1 AND deleted_at IS NULL",
                params![notebook_id],
                |r| r.get(0),
            )
            .optional()?;
        if live.is_none() {
            return Err(Error::NotFound(notebook_id.into()));
        }
        let id = uuid::Uuid::now_v7().to_string();
        let now = now_ms();
        self.conn.execute(
            "INSERT INTO note (id, notebook_id, title, body_md, status_id, created_at, updated_at)
             VALUES (?1, ?2, '', '', (SELECT id FROM status WHERE id = ?3), ?4, ?4)",
            // The subquery yields NULL if the default status was deleted, avoiding an FK error.
            params![id, notebook_id, DEFAULT_STATUS_ID, now],
        )?;
        self.get_note(&id)
    }

    pub fn update_note(&self, id: &str, title: &str, body_md: &str) -> Result<i64> {
        let now = now_ms();
        let n = self.conn.execute(
            "UPDATE note SET title = ?1, body_md = ?2, updated_at = ?3
             WHERE id = ?4 AND deleted_at IS NULL",
            params![title, body_md, now, id],
        )?;
        if n == 0 {
            return Err(Error::NotFound(id.into()));
        }
        Ok(now)
    }

    pub fn delete_note(&self, id: &str) -> Result<()> {
        let n = self.conn.execute(
            "UPDATE note SET deleted_at = ?1 WHERE id = ?2 AND deleted_at IS NULL",
            params![now_ms(), id],
        )?;
        if n == 0 {
            return Err(Error::NotFound(id.into()));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_defaults_to_active_status() {
        let db = Db::open_in_memory().unwrap();
        let nb = db.create_notebook(None, "NB").unwrap();
        let note = db.create_note(&nb.id).unwrap();
        assert_eq!(note.status_id.as_deref(), Some("status-active"));
        assert_eq!(note.notebook_id, nb.id);
    }

    #[test]
    fn create_in_missing_notebook_fails() {
        let db = Db::open_in_memory().unwrap();
        assert!(matches!(db.create_note("nope"), Err(Error::NotFound(_))));
    }

    #[test]
    fn update_roundtrips_markdown_verbatim() {
        let db = Db::open_in_memory().unwrap();
        let nb = db.create_notebook(None, "NB").unwrap();
        let note = db.create_note(&nb.id).unwrap();
        let body = "# 标题\n\n* a\n  - b\n\n```rs\nfn x() {}\n```\n";
        db.update_note(&note.id, "中文标题", body).unwrap();
        let got = db.get_note(&note.id).unwrap();
        assert_eq!(got.title, "中文标题");
        assert_eq!(got.body_md, body);
    }

    #[test]
    fn list_orders_by_recent_and_hides_deleted() {
        let db = Db::open_in_memory().unwrap();
        let nb = db.create_notebook(None, "NB").unwrap();
        let a = db.create_note(&nb.id).unwrap();
        let b = db.create_note(&nb.id).unwrap();
        let c = db.create_note(&nb.id).unwrap();
        // Force distinct timestamps so ordering is deterministic.
        for (id, t) in [(&a.id, 100), (&b.id, 300), (&c.id, 200)] {
            db.conn
                .execute("UPDATE note SET updated_at = ?1 WHERE id = ?2", params![t, id])
                .unwrap();
        }
        let ids: Vec<_> = db.list_notes(&nb.id).unwrap().into_iter().map(|n| n.id).collect();
        assert_eq!(ids, [b.id.clone(), c.id.clone(), a.id.clone()]);
        db.delete_note(&b.id).unwrap();
        assert_eq!(db.list_notes(&nb.id).unwrap().len(), 2);
        assert!(matches!(db.get_note(&b.id), Err(Error::NotFound(_))));
        assert!(matches!(db.delete_note(&b.id), Err(Error::NotFound(_))));
    }

    #[test]
    fn snippet_is_truncated() {
        let db = Db::open_in_memory().unwrap();
        let nb = db.create_notebook(None, "NB").unwrap();
        let note = db.create_note(&nb.id).unwrap();
        db.update_note(&note.id, "t", &"x".repeat(1000)).unwrap();
        assert_eq!(db.list_notes(&nb.id).unwrap()[0].snippet.chars().count(), 200);
    }
}
