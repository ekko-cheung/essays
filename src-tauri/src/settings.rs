//! App settings, stored as JSON in the app data dir (not in the notes database,
//! so they are per-device and never synced).

use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum CloseAction {
    /// Closing the window quits the app.
    #[default]
    Quit,
    /// Closing the window minimizes it; the app keeps running.
    Minimize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SyncSettings {
    pub enabled: bool,
    pub account_id: String,
    pub database_id: String,
    pub api_token: String,
    pub auto_sync: bool,
    pub interval_minutes: u32,
}

impl Default for SyncSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            account_id: String::new(),
            database_id: String::new(),
            api_token: String::new(),
            auto_sync: true,
            interval_minutes: 5,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub close_action: CloseAction,
    pub sync: SyncSettings,
}

impl Settings {
    /// A missing or unreadable file yields defaults rather than blocking startup.
    pub fn load(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        let json = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, json).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            // Holds an API token: keep it private to the user.
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600));
        }
        std::fs::rename(&tmp, path).map_err(|e| e.to_string())
    }
}

/// What the frontend sees: the token itself never leaves the backend.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsView {
    pub close_action: CloseAction,
    pub sync_enabled: bool,
    pub account_id: String,
    pub database_id: String,
    pub has_token: bool,
    pub auto_sync: bool,
    pub interval_minutes: u32,
}

impl From<&Settings> for SettingsView {
    fn from(s: &Settings) -> Self {
        Self {
            close_action: s.close_action,
            sync_enabled: s.sync.enabled,
            account_id: s.sync.account_id.clone(),
            database_id: s.sync.database_id.clone(),
            has_token: !s.sync.api_token.is_empty(),
            auto_sync: s.sync.auto_sync,
            interval_minutes: s.sync.interval_minutes,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsInput {
    pub close_action: CloseAction,
    pub sync_enabled: bool,
    pub account_id: String,
    pub database_id: String,
    /// `None` keeps the stored token.
    pub api_token: Option<String>,
    pub auto_sync: bool,
    pub interval_minutes: u32,
}

impl Settings {
    pub fn apply(&mut self, i: SettingsInput) {
        self.close_action = i.close_action;
        self.sync.enabled = i.sync_enabled;
        self.sync.account_id = i.account_id.trim().to_string();
        self.sync.database_id = i.database_id.trim().to_string();
        if let Some(t) = i.api_token {
            self.sync.api_token = t.trim().to_string();
        }
        self.sync.auto_sync = i.auto_sync;
        self.sync.interval_minutes = i.interval_minutes.clamp(1, 24 * 60);
    }
}
