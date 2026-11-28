//! App settings, stored as JSON in the app data dir (not in the notes database,
//! so they are per-device and never synced).

use serde::{Deserialize, Serialize};
use std::{io::Write, path::Path};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum CloseAction {
    /// Closing the window quits the app.
    #[default]
    Quit,
    /// The app keeps running in the system tray after the window closes.
    /// The serialized name stays `minimize` so existing settings files keep working.
    #[serde(rename = "minimize")]
    MinimizeToTray,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub close_action: CloseAction,
    /// Insert matching closers for brackets and fenced code blocks while typing.
    pub auto_close_brackets: bool,
    pub sync: SyncSettings,
    pub appearance: Appearance,
    pub markdown: MarkdownTypography,
    pub view: View,
    pub include_descendants: bool,
    pub auto_check_updates: bool,
    pub dismissed_version: Option<String>,
    pub sidebar_width: Option<f32>,
    pub list_width: Option<f32>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            close_action: CloseAction::default(),
            // On by default; settings files written before this option existed
            // also get the feature when deserialized.
            auto_close_brackets: true,
            sync: SyncSettings::default(),
            appearance: Appearance::System,
            markdown: MarkdownTypography::default(),
            view: View::Split,
            include_descendants: true,
            auto_check_updates: true,
            dismissed_version: None,
            sidebar_width: None,
            list_width: None,
        }
    }
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
        let dir = path
            .parent()
            .ok_or("Settings path has no parent directory")?;
        let mut tmp = tempfile::NamedTempFile::new_in(dir).map_err(|e| e.to_string())?;
        tmp.write_all(json.as_bytes()).map_err(|e| e.to_string())?;
        tmp.as_file().sync_all().map_err(|e| e.to_string())?;
        tmp.persist(path).map_err(|e| e.to_string())?;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Appearance {
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct MarkdownTypography {
    pub font_family: Option<String>,
    pub font_size: u16,
}

impl MarkdownTypography {
    pub const FONT_SIZES: std::ops::RangeInclusive<u16> = 12..=32;

    pub fn font_size(&self) -> u16 {
        self.font_size
            .clamp(*Self::FONT_SIZES.start(), *Self::FONT_SIZES.end())
    }
}

impl Default for MarkdownTypography {
    fn default() -> Self {
        Self {
            font_family: None,
            font_size: 18,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum View {
    Live,
    Source,
    #[default]
    Split,
    Preview,
}

#[derive(Debug)]
pub struct SettingsInput {
    pub close_action: CloseAction,
    pub auto_close_brackets: bool,
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
        self.auto_close_brackets = i.auto_close_brackets;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_close_defaults_to_on_for_new_and_old_settings() {
        assert!(Settings::default().auto_close_brackets);
        // A file written before the option existed should still get the feature.
        let old: Settings =
            serde_json::from_str(r#"{"closeAction":"minimize","sync":{"enabled":true}}"#).unwrap();
        assert_eq!(old.close_action, CloseAction::MinimizeToTray);
        assert!(old.auto_close_brackets);
    }

    #[test]
    fn apply_updates_auto_close() {
        let mut settings = Settings::default();
        settings.apply(SettingsInput {
            close_action: CloseAction::Quit,
            auto_close_brackets: false,
            sync_enabled: false,
            account_id: String::new(),
            database_id: String::new(),
            api_token: None,
            auto_sync: true,
            interval_minutes: 5,
        });
        assert!(!settings.auto_close_brackets);
    }

    #[test]
    fn markdown_typography_defaults_for_existing_settings_and_persists() {
        let old: Settings = serde_json::from_str(r#"{"appearance":"dark"}"#).unwrap();
        assert_eq!(old.markdown.font_size(), 18);
        assert_eq!(old.markdown.font_family, None);

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let custom = Settings {
            markdown: MarkdownTypography {
                font_family: Some("Georgia".into()),
                font_size: 22,
            },
            ..old
        };
        custom.save(&path).unwrap();
        assert_eq!(Settings::load(&path).markdown, custom.markdown);
    }
}
