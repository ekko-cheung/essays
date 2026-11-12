use essays_core::{
    note::{Note, NoteSummary},
    notebook::Notebook,
    query::NoteQuery,
    status::Status,
    tag::{Tag, DEFAULT_TAG_COLOR},
    Db,
};
use serde::Serialize;
use settings::{CloseAction, Settings, SettingsInput, SettingsView};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use tauri::{Manager, State};

mod cloud;
mod settings;

struct AppState {
    db: Mutex<Db>,
    settings: Mutex<Settings>,
    settings_path: PathBuf,
    syncing: AtomicBool,
}

type CmdResult<T> = Result<T, String>;

fn err(e: essays_core::Error) -> String {
    e.to_string()
}

#[tauri::command]
fn list_notebooks(state: State<AppState>) -> CmdResult<Vec<Notebook>> {
    state.db.lock().unwrap().list_notebooks().map_err(err)
}

#[tauri::command]
fn create_notebook(
    state: State<AppState>,
    parent_id: Option<String>,
    name: String,
) -> CmdResult<Notebook> {
    state
        .db
        .lock()
        .unwrap()
        .create_notebook(parent_id.as_deref(), &name)
        .map_err(err)
}

#[tauri::command]
fn rename_notebook(state: State<AppState>, id: String, name: String) -> CmdResult<()> {
    state.db.lock().unwrap().rename_notebook(&id, &name).map_err(err)
}

#[tauri::command]
fn move_notebook(
    state: State<AppState>,
    id: String,
    new_parent_id: Option<String>,
    index: usize,
) -> CmdResult<()> {
    state
        .db
        .lock()
        .unwrap()
        .move_notebook(&id, new_parent_id.as_deref(), index)
        .map_err(err)
}

#[tauri::command]
fn delete_notebook(state: State<AppState>, id: String) -> CmdResult<()> {
    state.db.lock().unwrap().delete_notebook(&id).map_err(err)
}

#[tauri::command]
fn query_notes(state: State<AppState>, query: NoteQuery) -> CmdResult<Vec<NoteSummary>> {
    state.db.lock().unwrap().query_notes(&query).map_err(err)
}

#[tauri::command]
fn get_note(state: State<AppState>, id: String) -> CmdResult<Note> {
    state.db.lock().unwrap().get_note(&id).map_err(err)
}

#[tauri::command]
fn create_note(state: State<AppState>, notebook_id: String) -> CmdResult<Note> {
    state.db.lock().unwrap().create_note(&notebook_id).map_err(err)
}

/// Returns the new `updatedAt` timestamp.
#[tauri::command]
fn update_note(state: State<AppState>, id: String, title: String, body_md: String) -> CmdResult<i64> {
    state
        .db
        .lock()
        .unwrap()
        .update_note(&id, &title, &body_md)
        .map_err(err)
}

#[tauri::command]
fn delete_note(state: State<AppState>, id: String) -> CmdResult<()> {
    state.db.lock().unwrap().delete_note(&id).map_err(err)
}

#[tauri::command]
fn list_statuses(state: State<AppState>) -> CmdResult<Vec<Status>> {
    state.db.lock().unwrap().list_statuses().map_err(err)
}

#[tauri::command]
fn create_status(state: State<AppState>, name: String, color: String) -> CmdResult<Status> {
    state.db.lock().unwrap().create_status(&name, &color).map_err(err)
}

#[tauri::command]
fn update_status(state: State<AppState>, id: String, name: String, color: String) -> CmdResult<()> {
    state.db.lock().unwrap().update_status(&id, &name, &color).map_err(err)
}

#[tauri::command]
fn delete_status(state: State<AppState>, id: String) -> CmdResult<()> {
    state.db.lock().unwrap().delete_status(&id).map_err(err)
}

#[tauri::command]
fn list_tags(state: State<AppState>) -> CmdResult<Vec<Tag>> {
    state.db.lock().unwrap().list_tags().map_err(err)
}

#[tauri::command]
fn create_tag(state: State<AppState>, name: String, color: Option<String>) -> CmdResult<Tag> {
    let color = color.unwrap_or_else(|| DEFAULT_TAG_COLOR.to_string());
    state.db.lock().unwrap().create_tag(&name, &color).map_err(err)
}

#[tauri::command]
fn update_tag(state: State<AppState>, id: String, name: String, color: String) -> CmdResult<()> {
    state.db.lock().unwrap().update_tag(&id, &name, &color).map_err(err)
}

#[tauri::command]
fn delete_tag(state: State<AppState>, id: String) -> CmdResult<()> {
    state.db.lock().unwrap().delete_tag(&id).map_err(err)
}

#[tauri::command]
fn set_note_status(
    state: State<AppState>,
    note_id: String,
    status_id: Option<String>,
) -> CmdResult<()> {
    state
        .db
        .lock()
        .unwrap()
        .set_note_status(&note_id, status_id.as_deref())
        .map_err(err)
}

#[tauri::command]
fn set_note_tags(state: State<AppState>, note_id: String, tag_ids: Vec<String>) -> CmdResult<()> {
    state.db.lock().unwrap().set_note_tags(&note_id, &tag_ids).map_err(err)
}

#[tauri::command]
fn get_settings(state: State<AppState>) -> SettingsView {
    SettingsView::from(&*state.settings.lock().unwrap())
}

#[tauri::command]
fn save_settings(state: State<AppState>, input: SettingsInput) -> CmdResult<SettingsView> {
    let mut s = state.settings.lock().unwrap();
    let mut next = s.clone();
    next.apply(input);
    next.save(&state.settings_path)?;
    *s = next;
    Ok(SettingsView::from(&*s))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SyncStatus {
    last_sync_at: Option<i64>,
    pending: i64,
}

#[tauri::command]
fn sync_status(state: State<AppState>) -> CmdResult<SyncStatus> {
    let db = state.db.lock().unwrap();
    Ok(SyncStatus {
        last_sync_at: db.sync_state("last_sync_at").map_err(err)?.and_then(|v| v.parse().ok()),
        pending: db.dirty_count().map_err(err)?,
    })
}

/// Verifies the saved credentials and prepares the remote tables.
#[tauri::command]
async fn sync_test(state: State<'_, AppState>) -> CmdResult<()> {
    let cfg = state.settings.lock().unwrap().sync.clone();
    cloud::test_connection(&cfg).await
}

#[tauri::command]
async fn sync_now(state: State<'_, AppState>) -> CmdResult<cloud::SyncReport> {
    let cfg = state.settings.lock().unwrap().sync.clone();
    if !cfg.enabled {
        return Err("Sync is turned off.".into());
    }
    if state.syncing.swap(true, Ordering::SeqCst) {
        return Err("A sync is already running.".into());
    }
    let result = cloud::run(&cfg, &state.db).await;
    state.syncing.store(false, Ordering::SeqCst);
    result
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            let db = Db::open(dir.join("essays.db"))?;
            let settings_path = dir.join("settings.json");
            app.manage(AppState {
                db: Mutex::new(db),
                settings: Mutex::new(Settings::load(&settings_path)),
                settings_path,
                syncing: AtomicBool::new(false),
            });

            // The window is created hidden and shown by the frontend as soon as
            // it runs. If the frontend never gets that far (script error, etc.),
            // don't leave the user with an invisible app.
            if let Some(window) = app.get_webview_window("main") {
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_secs(2));
                    let _ = window.show();
                });
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let minimize = window.state::<AppState>().settings.lock().unwrap().close_action
                    == CloseAction::Minimize;
                if minimize {
                    api.prevent_close();
                    let _ = window.minimize();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            list_notebooks,
            create_notebook,
            rename_notebook,
            move_notebook,
            delete_notebook,
            query_notes,
            get_note,
            create_note,
            update_note,
            delete_note,
            list_statuses,
            create_status,
            update_status,
            delete_status,
            list_tags,
            create_tag,
            update_tag,
            delete_tag,
            set_note_status,
            set_note_tags,
            get_settings,
            save_settings,
            sync_status,
            sync_test,
            sync_now
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
