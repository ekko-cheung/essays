use rusqlite::Connection;
use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("database error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("invalid operation: {0}")]
    Invalid(String),
}

pub type Result<T> = std::result::Result<T, Error>;

const MIGRATIONS: &[&str] = &[
    include_str!("../migrations/0001_init.sql"),
    include_str!("../migrations/0002_drop_fts.sql"),
];

pub struct Db {
    pub conn: Connection,
}

impl Db {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        Self::init(Connection::open(path)?)
    }

    pub fn open_in_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self> {
        conn.pragma_update(None, "foreign_keys", "ON")?;
        let db = Db { conn };
        db.migrate()?;
        Ok(db)
    }

    fn migrate(&self) -> Result<()> {
        let current: usize = self
            .conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))?;
        for (i, sql) in MIGRATIONS.iter().enumerate().skip(current) {
            self.conn.execute_batch(&format!(
                "BEGIN; {sql}; PRAGMA user_version = {}; COMMIT;",
                i + 1
            ))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upgrades_an_existing_v1_database_keeping_data() {
        let path = std::env::temp_dir().join(format!("essays-test-{}.db", uuid::Uuid::now_v7()));
        {
            // Simulate a database created by the first release.
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(MIGRATIONS[0]).unwrap();
            conn.pragma_update(None, "user_version", 1).unwrap();
            conn.execute(
                "INSERT INTO notebook (id, name, created_at, updated_at) VALUES ('nb', 'NB', 0, 0)",
                [],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO note (id, notebook_id, title, created_at, updated_at) VALUES ('n', 'nb', 'kept', 0, 0)",
                [],
            )
            .unwrap();
        }
        let db = Db::open(&path).unwrap();
        let version: i64 = db.conn.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
        assert_eq!(version as usize, MIGRATIONS.len());
        assert_eq!(db.get_note("n").unwrap().title, "kept");
        let fts: i64 = db
            .conn
            .query_row("SELECT COUNT(*) FROM sqlite_master WHERE name LIKE 'note_fts%'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(fts, 0);
        // Writes still work now that the FTS triggers are gone.
        db.update_note("n", "edited", "body").unwrap();
        drop(db);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn migrates_and_seeds_default_statuses() {
        let db = Db::open_in_memory().unwrap();
        let names: Vec<String> = db
            .conn
            .prepare("SELECT name FROM status ORDER BY sort_order")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<std::result::Result<_, _>>()
            .unwrap();
        assert_eq!(names, ["Active", "On Hold", "Complete", "Dropped"]);
    }
}
