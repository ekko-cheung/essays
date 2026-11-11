//! Sync with a Cloudflare D1 database through the Cloudflare REST API.
//! The merge rules live in `essays_core::sync`; this module only moves rows.

use crate::settings::SyncSettings;
use essays_core::sync::{
    pull_statement, remote_schema, upsert_statement, ApplyStats, Changes, Table,
};
use essays_core::Db;
use serde::Serialize;
use serde_json::{json, Value};
use std::sync::Mutex;
use std::time::Duration;

/// Statements per request; keeps each request comfortably small.
const BATCH: usize = 25;
const PUSH_PAGE: usize = 50;
const PULL_PAGE: usize = 100;
/// Safety valve so a note being edited non-stop can't keep one sync running forever.
const MAX_PUSH_ROUNDS: usize = 200;

pub struct D1 {
    http: reqwest::Client,
    url: String,
    token: String,
}

type Stmt = (String, Vec<Value>);

impl D1 {
    pub fn new(cfg: &SyncSettings) -> Result<Self, String> {
        if cfg.account_id.is_empty() || cfg.database_id.is_empty() || cfg.api_token.is_empty() {
            return Err("Fill in the account ID, database ID and API token first.".into());
        }
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .map_err(|e| e.to_string())?;
        Ok(Self {
            http,
            url: format!(
                "https://api.cloudflare.com/client/v4/accounts/{}/d1/database/{}/query",
                cfg.account_id, cfg.database_id
            ),
            token: cfg.api_token.clone(),
        })
    }

    /// Run statements as one D1 batch (a transaction). Returns each statement's rows.
    async fn batch(&self, stmts: &[Stmt]) -> Result<Vec<Vec<Value>>, String> {
        let body = json!({
            "batch": stmts.iter().map(|(sql, params)| json!({ "sql": sql, "params": params })).collect::<Vec<_>>()
        });
        let resp = self
            .http
            .post(&self.url)
            .bearer_auth(&self.token)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Could not reach Cloudflare: {e}"))?;
        let status = resp.status();
        let text = resp.text().await.map_err(|e| e.to_string())?;
        let v: Value = serde_json::from_str(&text)
            .map_err(|_| format!("Unexpected response from Cloudflare (HTTP {status})"))?;
        if !status.is_success() || v["success"] != json!(true) {
            let msg = v["errors"]
                .as_array()
                .map(|a| a.iter().filter_map(|e| e["message"].as_str()).collect::<Vec<_>>().join("; "))
                .filter(|m| !m.is_empty())
                .unwrap_or_else(|| format!("HTTP {status}"));
            return Err(match status.as_u16() {
                401 | 403 => format!("Cloudflare rejected the API token ({msg}). It needs the \"D1 Edit\" permission."),
                404 => format!("D1 database not found ({msg}). Check the account and database IDs."),
                _ => format!("Cloudflare D1 error: {msg}"),
            });
        }
        let results = v["result"].as_array().ok_or("Malformed D1 response")?;
        Ok(results
            .iter()
            .map(|r| r["results"].as_array().cloned().unwrap_or_default())
            .collect())
    }

    pub async fn ensure_schema(&self) -> Result<(), String> {
        let stmts: Vec<Stmt> = remote_schema().into_iter().map(|s| (s, vec![])).collect();
        self.batch(&stmts).await.map(|_| ())
    }

    async fn pull_table(&self, table: Table, mut cursor: i64) -> Result<(Vec<Value>, i64), String> {
        let mut rows = Vec::new();
        let mut after = String::new();
        loop {
            let page = self
                .batch(&[pull_statement(table, cursor, &after, PULL_PAGE)])
                .await?
                .into_iter()
                .next()
                .unwrap_or_default();
            let n = page.len();
            if let Some(last) = page.last() {
                cursor = last["synced_at"].as_i64().unwrap_or(cursor);
                after = last["id"].as_str().unwrap_or_default().to_string();
            }
            rows.extend(page);
            if n < PULL_PAGE {
                return Ok((rows, cursor));
            }
        }
    }

    async fn push(&self, c: &Changes) -> Result<(), String> {
        let mut stmts: Vec<Stmt> = Vec::with_capacity(c.len());
        macro_rules! add {
            ($table:expr, $rows:expr) => {
                for r in $rows {
                    let v = serde_json::to_value(r).map_err(|e| e.to_string())?;
                    stmts.push(upsert_statement($table, &v));
                }
            };
        }
        add!(Table::Notebook, &c.notebooks);
        add!(Table::Status, &c.statuses);
        add!(Table::Tag, &c.tags);
        add!(Table::Note, &c.notes);
        for chunk in stmts.chunks(BATCH) {
            self.batch(chunk).await?;
        }
        Ok(())
    }
}

#[derive(Debug, Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncReport {
    pub pulled: usize,
    pub pushed: usize,
    pub changed_note_ids: Vec<String>,
    pub at: i64,
}

fn lock<'a>(db: &'a Mutex<Db>) -> std::sync::MutexGuard<'a, Db> {
    db.lock().unwrap_or_else(|e| e.into_inner())
}

fn parse<T: serde::de::DeserializeOwned>(rows: Vec<Value>) -> Vec<T> {
    // A row this version can't read is skipped rather than failing the whole sync.
    rows.into_iter().filter_map(|r| serde_json::from_value(r).ok()).collect()
}

pub async fn test_connection(cfg: &SyncSettings) -> Result<(), String> {
    D1::new(cfg)?.ensure_schema().await
}

/// Pull what's new remotely, merge it, then upload local changes.
pub async fn run(cfg: &SyncSettings, db: &Mutex<Db>) -> Result<SyncReport, String> {
    let d1 = D1::new(cfg)?;
    d1.ensure_schema().await?;

    // Pointing at a different database invalidates everything we know about "synced".
    {
        let identity = format!("{}/{}", cfg.account_id, cfg.database_id);
        let db = lock(db);
        if db.sync_state("remote").map_err(|e| e.to_string())?.as_deref() != Some(identity.as_str()) {
            db.reset_sync().map_err(|e| e.to_string())?;
            db.set_sync_state("remote", &identity).map_err(|e| e.to_string())?;
        }
    }

    // Fetch children before parents: a row we see can then never reference a parent
    // that we fail to see (parents are always uploaded first).
    let mut incoming = Changes::default();
    let mut cursors = Vec::new();
    for table in [Table::Note, Table::Tag, Table::Status, Table::Notebook] {
        let from = lock(db).cursor(table).map_err(|e| e.to_string())?;
        let (rows, cursor) = d1.pull_table(table, from).await?;
        match table {
            Table::Note => incoming.notes = parse(rows),
            Table::Tag => incoming.tags = parse(rows),
            Table::Status => incoming.statuses = parse(rows),
            Table::Notebook => incoming.notebooks = parse(rows),
        }
        cursors.push((table, cursor));
    }
    let stats: ApplyStats = {
        let db = lock(db);
        let stats = db.apply_changes(&incoming).map_err(|e| e.to_string())?;
        for (t, c) in cursors {
            db.set_cursor(t, c).map_err(|e| e.to_string())?;
        }
        stats
    };

    let mut pushed = 0;
    for _ in 0..MAX_PUSH_ROUNDS {
        let pending = lock(db).pending_changes(PUSH_PAGE).map_err(|e| e.to_string())?;
        if pending.is_empty() {
            break;
        }
        d1.push(&pending).await?;
        lock(db).mark_pushed(&pending).map_err(|e| e.to_string())?;
        pushed += pending.len();
    }

    let at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    lock(db).set_sync_state("last_sync_at", &at.to_string()).map_err(|e| e.to_string())?;
    Ok(SyncReport { pulled: stats.applied, pushed, changed_note_ids: stats.changed_note_ids, at })
}
