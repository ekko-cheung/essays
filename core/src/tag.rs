use crate::{now_ms, valid_color, Db, Error, Result};
use rusqlite::params;
use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Tag {
    pub id: String,
    pub name: String,
    pub color: String,
    /// Live notes carrying this tag.
    pub note_count: i64,
}

pub const DEFAULT_TAG_COLOR: &str = "#64748b";

impl Db {
    pub fn list_tags(&self) -> Result<Vec<Tag>> {
        let mut stmt = self.conn.prepare(
            "SELECT t.id, t.name, t.color,
                    (SELECT COUNT(*) FROM note_tag nt JOIN note n ON n.id = nt.note_id
                     WHERE nt.tag_id = t.id AND n.deleted_at IS NULL)
             FROM tag t ORDER BY lower(t.name)",
        )?;
        let rows = stmt
            .query_map([], |r| {
                Ok(Tag {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    color: r.get(2)?,
                    note_count: r.get(3)?,
                })
            })?
            .collect::<std::result::Result<_, _>>()?;
        Ok(rows)
    }

    pub fn create_tag(&self, name: &str, color: &str) -> Result<Tag> {
        let name = name.trim();
        self.check_tag(name, color, None)?;
        let id = uuid::Uuid::now_v7().to_string();
        self.conn.execute(
            "INSERT INTO tag (id, name, color) VALUES (?1, ?2, ?3)",
            params![id, name, color],
        )?;
        Ok(Tag { id, name: name.into(), color: color.into(), note_count: 0 })
    }

    pub fn update_tag(&self, id: &str, name: &str, color: &str) -> Result<()> {
        let name = name.trim();
        self.check_tag(name, color, Some(id))?;
        let n = self.conn.execute(
            "UPDATE tag SET name = ?1, color = ?2 WHERE id = ?3",
            params![name, color, id],
        )?;
        if n == 0 {
            return Err(Error::NotFound(id.into()));
        }
        Ok(())
    }

    /// Removes the tag from every note (note_tag rows cascade).
    pub fn delete_tag(&self, id: &str) -> Result<()> {
        let n = self.conn.execute("DELETE FROM tag WHERE id = ?1", params![id])?;
        if n == 0 {
            return Err(Error::NotFound(id.into()));
        }
        Ok(())
    }

    pub fn set_note_status(&self, note_id: &str, status_id: Option<&str>) -> Result<()> {
        let n = self.conn.execute(
            "UPDATE note SET status_id = ?1, updated_at = ?2 WHERE id = ?3 AND deleted_at IS NULL",
            params![status_id, now_ms(), note_id],
        )?;
        if n == 0 {
            return Err(Error::NotFound(note_id.into()));
        }
        Ok(())
    }

    /// Replace the note's whole tag set.
    pub fn set_note_tags(&self, note_id: &str, tag_ids: &[String]) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        let n = tx.execute(
            "UPDATE note SET updated_at = ?1 WHERE id = ?2 AND deleted_at IS NULL",
            params![now_ms(), note_id],
        )?;
        if n == 0 {
            return Err(Error::NotFound(note_id.into()));
        }
        tx.execute("DELETE FROM note_tag WHERE note_id = ?1", params![note_id])?;
        for tag_id in tag_ids {
            tx.execute(
                "INSERT OR IGNORE INTO note_tag (note_id, tag_id) VALUES (?1, ?2)",
                params![note_id, tag_id],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    fn check_tag(&self, name: &str, color: &str, except_id: Option<&str>) -> Result<()> {
        if name.is_empty() {
            return Err(Error::Invalid("tag name must not be empty".into()));
        }
        if !valid_color(color) {
            return Err(Error::Invalid(format!("invalid color: {color}")));
        }
        let dup: bool = self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM tag WHERE lower(name) = lower(?1) AND id IS NOT ?2)",
            params![name, except_id],
            |r| r.get(0),
        )?;
        if dup {
            return Err(Error::Invalid(format!("tag \"{name}\" already exists")));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> (Db, String) {
        let db = Db::open_in_memory().unwrap();
        let nb = db.create_notebook(None, "NB").unwrap();
        let note = db.create_note(&nb.id).unwrap();
        (db, note.id)
    }

    #[test]
    fn tag_crud_and_uniqueness() {
        let db = Db::open_in_memory().unwrap();
        let t = db.create_tag(" Rust ", DEFAULT_TAG_COLOR).unwrap();
        assert_eq!(t.name, "Rust");
        assert!(matches!(db.create_tag("rust", DEFAULT_TAG_COLOR), Err(Error::Invalid(_))));
        assert!(matches!(db.create_tag("", DEFAULT_TAG_COLOR), Err(Error::Invalid(_))));
        db.update_tag(&t.id, "Rustlang", "#ff0000").unwrap();
        assert_eq!(db.list_tags().unwrap()[0].name, "Rustlang");
        db.delete_tag(&t.id).unwrap();
        assert!(db.list_tags().unwrap().is_empty());
    }

    #[test]
    fn set_tags_replaces_and_counts_only_live_notes() {
        let (db, note_id) = setup();
        let a = db.create_tag("a", DEFAULT_TAG_COLOR).unwrap();
        let b = db.create_tag("b", DEFAULT_TAG_COLOR).unwrap();
        db.set_note_tags(&note_id, &[a.id.clone(), b.id.clone(), a.id.clone()]).unwrap();
        assert_eq!(db.get_note(&note_id).unwrap().tag_ids.len(), 2);
        db.set_note_tags(&note_id, &[b.id.clone()]).unwrap();
        assert_eq!(db.get_note(&note_id).unwrap().tag_ids, [b.id.clone()]);
        let counts: Vec<_> = db.list_tags().unwrap().into_iter().map(|t| (t.name, t.note_count)).collect();
        assert_eq!(counts, [("a".to_string(), 0), ("b".to_string(), 1)]);
        db.delete_note(&note_id).unwrap();
        assert_eq!(db.list_tags().unwrap()[1].note_count, 0);
    }

    #[test]
    fn deleting_tag_detaches_from_notes() {
        let (db, note_id) = setup();
        let a = db.create_tag("a", DEFAULT_TAG_COLOR).unwrap();
        db.set_note_tags(&note_id, &[a.id.clone()]).unwrap();
        db.delete_tag(&a.id).unwrap();
        assert!(db.get_note(&note_id).unwrap().tag_ids.is_empty());
    }

    #[test]
    fn set_status_and_summary_carries_tags() {
        let (db, note_id) = setup();
        let nb_id = db.get_note(&note_id).unwrap().notebook_id;
        let a = db.create_tag("a", DEFAULT_TAG_COLOR).unwrap();
        db.set_note_tags(&note_id, &[a.id.clone()]).unwrap();
        db.set_note_status(&note_id, Some("status-complete")).unwrap();
        let s = &db.list_notes(&nb_id).unwrap()[0];
        assert_eq!(s.status_id.as_deref(), Some("status-complete"));
        assert_eq!(s.tag_ids, [a.id]);
        db.set_note_status(&note_id, None).unwrap();
        assert_eq!(db.get_note(&note_id).unwrap().status_id, None);
        assert!(matches!(db.set_note_status("nope", None), Err(Error::NotFound(_))));
    }
}
