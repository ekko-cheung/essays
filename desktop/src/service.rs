use crate::{cloud, settings::Settings};
use essays_core::Db;
use std::{
    path::{Path, PathBuf},
    sync::Mutex,
};

pub const APP_ID: &str = "com.essays.app";
pub type Result<T> = std::result::Result<T, String>;

pub struct Service {
    db: Mutex<Db>,
    settings: Mutex<Settings>,
    settings_path: PathBuf,
    sync: tokio::sync::Mutex<()>,
}

impl Service {
    pub fn data_dir() -> Result<PathBuf> {
        #[cfg(debug_assertions)]
        if let Some(dir) = std::env::var_os("ESSAYS_DEV_DATA_DIR") {
            return Ok(PathBuf::from(dir));
        }
        dirs::data_dir()
            .map(|dir| dir.join(APP_ID))
            .ok_or_else(|| "Could not locate the Essays data directory.".into())
    }

    pub fn open(dir: &Path) -> Result<Self> {
        let path = dir.join("essays.db");
        let db = Db::open(&path)
            .map_err(|error| format!("Could not open {}. {error}", path.display()))?;
        let settings_path = dir.join("settings.json");
        Ok(Self {
            db: Mutex::new(db),
            settings: Mutex::new(Settings::load(&settings_path)),
            settings_path,
            sync: tokio::sync::Mutex::new(()),
        })
    }

    pub fn db<T>(&self, operation: impl FnOnce(&Db) -> essays_core::Result<T>) -> Result<T> {
        operation(&self.db.lock().unwrap()).map_err(|error| error.to_string())
    }

    pub fn settings(&self) -> Settings {
        self.settings.lock().unwrap().clone()
    }

    pub fn save_settings(&self, update: impl FnOnce(&mut Settings)) -> Result<()> {
        let mut settings = self.settings.lock().unwrap();
        let mut next = settings.clone();
        update(&mut next);
        next.save(&self.settings_path)?;
        *settings = next;
        Ok(())
    }

    pub async fn sync(&self) -> Result<cloud::SyncReport> {
        let _guard = self
            .sync
            .try_lock()
            .map_err(|_| "A sync is already running.")?;
        let config = self.settings().sync;
        if !config.enabled {
            return Err("Sync is turned off.".into());
        }
        cloud::run(&config, &self.db).await
    }

    pub fn sync_status(&self) -> Result<(i64, Option<i64>)> {
        self.db(|db| {
            Ok((
                db.dirty_count()?,
                db.sync_state("last_sync_at")?.and_then(|v| v.parse().ok()),
            ))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::Service;
    use crate::settings::{CloseAction, View};

    #[test]
    fn opens_an_existing_profile_and_preserves_its_sync_state() {
        let profile = tempfile::tempdir().unwrap();
        let note = {
            let db = essays_core::Db::open(profile.path().join("essays.db")).unwrap();
            let parent = db.create_notebook(None, "Archive").unwrap();
            let notebook = db.create_notebook(Some(&parent.id), "Journal").unwrap();
            let note = db.create_note(&notebook.id).unwrap();
            db.update_note(&note.id, "草稿", "# Journal").unwrap();
            db.set_note_pinned(&note.id, true).unwrap();
            let tag = db.create_tag("work", "#64748b").unwrap();
            db.set_note_tags(&note.id, &[tag.id]).unwrap();
            db.set_sync_state("last_sync_at", "123456").unwrap();
            db.get_note(&note.id).unwrap()
        };
        std::fs::write(profile.path().join("settings.json"),
            r#"{"closeAction":"minimize","sync":{"enabled":true,"apiToken":"test-token","accountId":"test-account","databaseId":"test-db"}}"#)
            .unwrap();
        let app = Service::open(profile.path()).unwrap();
        assert_eq!(app.db(|db| db.get_note(&note.id)).unwrap(), note);
        let pending = app.sync_status().unwrap();
        assert!(pending.0 > 0);
        assert_eq!(pending.1, Some(123456));
        assert_eq!(app.settings().close_action, CloseAction::MinimizeToTray);
        assert_eq!(app.settings().view, View::Split);
        app.save_settings(|settings| settings.view = View::Preview)
            .unwrap();
        let reopened = Service::open(profile.path()).unwrap();
        assert_eq!(reopened.settings().view, View::Preview);
        assert_eq!(reopened.settings().sync.api_token, "test-token");
        assert_eq!(reopened.sync_status().unwrap(), pending);
    }
}
