use crate::{now_ms, Db, Error, Result};
use rusqlite::{params, OptionalExtension};
use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Notebook {
    pub id: String,
    pub parent_id: Option<String>,
    pub name: String,
    pub sort_order: i64,
}

impl Db {
    /// All live notebooks, flat; ordered so siblings are in display order.
    pub fn list_notebooks(&self) -> Result<Vec<Notebook>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, parent_id, name, sort_order FROM notebook
             WHERE deleted_at IS NULL ORDER BY sort_order, created_at",
        )?;
        let rows = stmt
            .query_map([], |r| {
                Ok(Notebook {
                    id: r.get(0)?,
                    parent_id: r.get(1)?,
                    name: r.get(2)?,
                    sort_order: r.get(3)?,
                })
            })?
            .collect::<std::result::Result<_, _>>()?;
        Ok(rows)
    }

    pub fn create_notebook(&self, parent_id: Option<&str>, name: &str) -> Result<Notebook> {
        let name = name.trim();
        if name.is_empty() {
            return Err(Error::Invalid("name must not be empty".into()));
        }
        if let Some(p) = parent_id {
            self.require_live(p)?;
        }
        let sort_order: i64 = self.conn.query_row(
            "SELECT COALESCE(MAX(sort_order) + 1, 0) FROM notebook
             WHERE parent_id IS ?1 AND deleted_at IS NULL",
            params![parent_id],
            |r| r.get(0),
        )?;
        let id = uuid::Uuid::now_v7().to_string();
        let now = now_ms();
        self.conn.execute(
            "INSERT INTO notebook (id, parent_id, name, sort_order, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
            params![id, parent_id, name, sort_order, now],
        )?;
        Ok(Notebook {
            id,
            parent_id: parent_id.map(String::from),
            name: name.into(),
            sort_order,
        })
    }

    pub fn rename_notebook(&self, id: &str, name: &str) -> Result<()> {
        let name = name.trim();
        if name.is_empty() {
            return Err(Error::Invalid("name must not be empty".into()));
        }
        let n = self.conn.execute(
            "UPDATE notebook SET name = ?1, updated_at = ?2 WHERE id = ?3 AND deleted_at IS NULL",
            params![name, now_ms(), id],
        )?;
        if n == 0 {
            return Err(Error::NotFound(id.into()));
        }
        Ok(())
    }

    /// Move `id` under `new_parent_id` (None = root) at `index` among its new siblings.
    pub fn move_notebook(&self, id: &str, new_parent_id: Option<&str>, index: usize) -> Result<()> {
        self.require_live(id)?;
        if let Some(p) = new_parent_id {
            self.require_live(p)?;
            // Reject moving into itself or its own descendant.
            let is_cycle: bool = self.conn.query_row(
                "WITH RECURSIVE sub(id) AS (
                     SELECT ?1 UNION ALL
                     SELECT n.id FROM notebook n JOIN sub ON n.parent_id = sub.id
                 ) SELECT EXISTS(SELECT 1 FROM sub WHERE id = ?2)",
                params![id, p],
                |r| r.get(0),
            )?;
            if is_cycle {
                return Err(Error::Invalid(
                    "cannot move a notebook into itself or its descendant".into(),
                ));
            }
        }
        let tx = self.conn.unchecked_transaction()?;
        let mut siblings: Vec<String> = tx
            .prepare(
                "SELECT id FROM notebook WHERE parent_id IS ?1 AND deleted_at IS NULL AND id != ?2
                 ORDER BY sort_order, created_at",
            )?
            .query_map(params![new_parent_id, id], |r| r.get(0))?
            .collect::<std::result::Result<_, _>>()?;
        siblings.insert(index.min(siblings.len()), id.to_string());
        tx.execute(
            "UPDATE notebook SET parent_id = ?1, updated_at = ?2 WHERE id = ?3",
            params![new_parent_id, now_ms(), id],
        )?;
        for (i, sid) in siblings.iter().enumerate() {
            tx.execute(
                "UPDATE notebook SET sort_order = ?1 WHERE id = ?2",
                params![i as i64, sid],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Soft-delete a notebook, all its descendants, and the notes inside them.
    pub fn delete_notebook(&self, id: &str) -> Result<()> {
        self.require_live(id)?;
        let now = now_ms();
        let tx = self.conn.unchecked_transaction()?;
        tx.execute(
            "WITH RECURSIVE sub(id) AS (
                 SELECT ?1 UNION ALL
                 SELECT n.id FROM notebook n JOIN sub ON n.parent_id = sub.id
             )
             UPDATE note SET deleted_at = ?2 WHERE deleted_at IS NULL
               AND notebook_id IN (SELECT id FROM sub)",
            params![id, now],
        )?;
        tx.execute(
            "WITH RECURSIVE sub(id) AS (
                 SELECT ?1 UNION ALL
                 SELECT n.id FROM notebook n JOIN sub ON n.parent_id = sub.id
             )
             UPDATE notebook SET deleted_at = ?2 WHERE deleted_at IS NULL
               AND id IN (SELECT id FROM sub)",
            params![id, now],
        )?;
        tx.commit()?;
        Ok(())
    }

    fn require_live(&self, id: &str) -> Result<()> {
        let found: Option<i64> = self
            .conn
            .query_row(
                "SELECT 1 FROM notebook WHERE id = ?1 AND deleted_at IS NULL",
                params![id],
                |r| r.get(0),
            )
            .optional()?;
        found.map(|_| ()).ok_or_else(|| Error::NotFound(id.into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(db: &Db, parent: Option<&str>) -> Vec<String> {
        db.list_notebooks()
            .unwrap()
            .into_iter()
            .filter(|n| n.parent_id.as_deref() == parent)
            .map(|n| n.name)
            .collect()
    }

    #[test]
    fn create_nested_and_list_in_order() {
        let db = Db::open_in_memory().unwrap();
        let a = db.create_notebook(None, "A").unwrap();
        db.create_notebook(None, "B").unwrap();
        db.create_notebook(Some(&a.id), "A1").unwrap();
        db.create_notebook(Some(&a.id), "A2").unwrap();
        assert_eq!(names(&db, None), ["A", "B"]);
        assert_eq!(names(&db, Some(&a.id)), ["A1", "A2"]);
    }

    #[test]
    fn rejects_empty_name_and_missing_parent() {
        let db = Db::open_in_memory().unwrap();
        assert!(matches!(db.create_notebook(None, "  "), Err(Error::Invalid(_))));
        assert!(matches!(
            db.create_notebook(Some("nope"), "x"),
            Err(Error::NotFound(_))
        ));
    }

    #[test]
    fn rename_works() {
        let db = Db::open_in_memory().unwrap();
        let a = db.create_notebook(None, "A").unwrap();
        db.rename_notebook(&a.id, "Z").unwrap();
        assert_eq!(names(&db, None), ["Z"]);
    }

    #[test]
    fn move_reorders_and_reparents() {
        let db = Db::open_in_memory().unwrap();
        let a = db.create_notebook(None, "A").unwrap();
        let b = db.create_notebook(None, "B").unwrap();
        let c = db.create_notebook(None, "C").unwrap();
        db.move_notebook(&c.id, None, 0).unwrap();
        assert_eq!(names(&db, None), ["C", "A", "B"]);
        db.move_notebook(&b.id, Some(&a.id), 0).unwrap();
        assert_eq!(names(&db, None), ["C", "A"]);
        assert_eq!(names(&db, Some(&a.id)), ["B"]);
    }

    #[test]
    fn move_into_own_descendant_is_rejected() {
        let db = Db::open_in_memory().unwrap();
        let a = db.create_notebook(None, "A").unwrap();
        let a1 = db.create_notebook(Some(&a.id), "A1").unwrap();
        assert!(matches!(
            db.move_notebook(&a.id, Some(&a1.id), 0),
            Err(Error::Invalid(_))
        ));
        assert!(matches!(
            db.move_notebook(&a.id, Some(&a.id), 0),
            Err(Error::Invalid(_))
        ));
    }

    #[test]
    fn delete_removes_subtree_and_its_notes() {
        let db = Db::open_in_memory().unwrap();
        let a = db.create_notebook(None, "A").unwrap();
        let a1 = db.create_notebook(Some(&a.id), "A1").unwrap();
        db.create_notebook(None, "B").unwrap();
        db.conn
            .execute(
                "INSERT INTO note (id, notebook_id, created_at, updated_at) VALUES ('n1', ?1, 0, 0)",
                params![a1.id],
            )
            .unwrap();
        db.delete_notebook(&a.id).unwrap();
        assert_eq!(names(&db, None), ["B"]);
        let live: i64 = db
            .conn
            .query_row("SELECT COUNT(*) FROM note WHERE deleted_at IS NULL", [], |r| r.get(0))
            .unwrap();
        assert_eq!(live, 0);
        assert!(matches!(db.delete_notebook(&a.id), Err(Error::NotFound(_))));
    }
}
