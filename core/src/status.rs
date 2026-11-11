use crate::{valid_color, Db, Error, Result};
use rusqlite::params;
use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub id: String,
    pub name: String,
    pub color: String,
    pub sort_order: i64,
    pub is_default: bool,
    /// Live notes currently using this status.
    pub note_count: i64,
}

impl Db {
    pub fn list_statuses(&self) -> Result<Vec<Status>> {
        let mut stmt = self.conn.prepare(
            "SELECT s.id, s.name, s.color, s.sort_order, s.is_default,
                    (SELECT COUNT(*) FROM note n WHERE n.status_id = s.id AND n.deleted_at IS NULL)
             FROM status s ORDER BY s.sort_order, s.name",
        )?;
        let rows = stmt
            .query_map([], |r| {
                Ok(Status {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    color: r.get(2)?,
                    sort_order: r.get(3)?,
                    is_default: r.get::<_, i64>(4)? != 0,
                    note_count: r.get(5)?,
                })
            })?
            .collect::<std::result::Result<_, _>>()?;
        Ok(rows)
    }

    pub fn create_status(&self, name: &str, color: &str) -> Result<Status> {
        let name = name.trim();
        self.check_status(name, color, None)?;
        let id = uuid::Uuid::now_v7().to_string();
        self.conn.execute(
            "INSERT INTO status (id, name, color, sort_order, is_default)
             VALUES (?1, ?2, ?3, (SELECT COALESCE(MAX(sort_order) + 1, 0) FROM status), 0)",
            params![id, name, color],
        )?;
        self.list_statuses()?
            .into_iter()
            .find(|s| s.id == id)
            .ok_or_else(|| Error::NotFound(id))
    }

    pub fn update_status(&self, id: &str, name: &str, color: &str) -> Result<()> {
        let name = name.trim();
        self.check_status(name, color, Some(id))?;
        let n = self.conn.execute(
            "UPDATE status SET name = ?1, color = ?2 WHERE id = ?3",
            params![name, color, id],
        )?;
        if n == 0 {
            return Err(Error::NotFound(id.into()));
        }
        Ok(())
    }

    /// Notes that used this status become status-less (ON DELETE SET NULL).
    pub fn delete_status(&self, id: &str) -> Result<()> {
        let n = self.conn.execute("DELETE FROM status WHERE id = ?1", params![id])?;
        if n == 0 {
            return Err(Error::NotFound(id.into()));
        }
        Ok(())
    }

    fn check_status(&self, name: &str, color: &str, except_id: Option<&str>) -> Result<()> {
        if name.is_empty() {
            return Err(Error::Invalid("status name must not be empty".into()));
        }
        if !valid_color(color) {
            return Err(Error::Invalid(format!("invalid color: {color}")));
        }
        let dup: bool = self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM status WHERE lower(name) = lower(?1) AND id IS NOT ?2)",
            params![name, except_id],
            |r| r.get(0),
        )?;
        if dup {
            return Err(Error::Invalid(format!("status \"{name}\" already exists")));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_listed_in_order() {
        let db = Db::open_in_memory().unwrap();
        let names: Vec<_> = db.list_statuses().unwrap().into_iter().map(|s| s.name).collect();
        assert_eq!(names, ["Active", "On Hold", "Complete", "Dropped"]);
    }

    #[test]
    fn create_update_delete_custom_status() {
        let db = Db::open_in_memory().unwrap();
        let s = db.create_status("  Review ", "#ff00aa").unwrap();
        assert_eq!(s.name, "Review");
        assert!(!s.is_default);
        assert_eq!(s.sort_order, 4);
        db.update_status(&s.id, "In Review", "#00ff00").unwrap();
        let got = db.list_statuses().unwrap().pop().unwrap();
        assert_eq!((got.name.as_str(), got.color.as_str()), ("In Review", "#00ff00"));
        db.delete_status(&s.id).unwrap();
        assert_eq!(db.list_statuses().unwrap().len(), 4);
        assert!(matches!(db.delete_status(&s.id), Err(Error::NotFound(_))));
    }

    #[test]
    fn rejects_duplicate_empty_and_bad_color() {
        let db = Db::open_in_memory().unwrap();
        assert!(matches!(db.create_status("active", "#000000"), Err(Error::Invalid(_))));
        assert!(matches!(db.create_status(" ", "#000000"), Err(Error::Invalid(_))));
        assert!(matches!(db.create_status("X", "red"), Err(Error::Invalid(_))));
        // Renaming to its own name (different case) is fine.
        db.update_status("status-active", "ACTIVE", "#22c55e").unwrap();
    }

    #[test]
    fn deleting_status_clears_it_on_notes() {
        let db = Db::open_in_memory().unwrap();
        let nb = db.create_notebook(None, "NB").unwrap();
        let note = db.create_note(&nb.id).unwrap();
        db.delete_status("status-active").unwrap();
        assert_eq!(db.get_note(&note.id).unwrap().status_id, None);
        // New notes still work when the default status is gone.
        assert_eq!(db.create_note(&nb.id).unwrap().status_id, None);
    }
}
