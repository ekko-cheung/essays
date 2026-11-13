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
    /// Set when the user closes the window to the tray, so the startup
    /// "make sure the window is visible" fallback doesn't pop it back up.
    hidden_to_tray: AtomicBool,
}

type CmdResult<T> = Result<T, String>;

fn err(e: essays_core::Error) -> String {
    e.to_string()
}

#[cfg(desktop)]
const TRAY_ID: &str = "essays-tray";

/// Brings the main window back from the tray, unminimized and focused.
#[cfg(desktop)]
fn show_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// Creates the tray icon with an "Open" / "Quit" menu. Left-clicking the icon
/// (Windows and macOS) restores the window; the menu opens on right click.
#[cfg(desktop)]
fn setup_tray(app: &tauri::AppHandle) -> tauri::Result<()> {
    use tauri::menu::{Menu, MenuItem};
    use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

    let open = MenuItem::with_id(app, "open", "Open Essays", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Essays", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &quit])?;

    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip("Essays")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => show_main_window(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        });

    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }

    builder.build(app)?;
    Ok(())
}

/// Adds or removes the tray icon to match the close action: it only exists
/// while closing the window hides the app to the tray.
#[cfg(desktop)]
fn set_tray_enabled(app: &tauri::AppHandle, enabled: bool) -> tauri::Result<()> {
    let id = tauri::tray::TrayIconId::new(TRAY_ID);
    let exists = app.tray_by_id(&id).is_some();
    if enabled && !exists {
        setup_tray(app)?;
    } else if !enabled && exists {
        // Dropping the handle returned by `remove_tray_by_id` removes the icon.
        let _ = app.remove_tray_by_id(&id);
    }
    Ok(())
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
fn save_settings(
    app: tauri::AppHandle,
    state: State<AppState>,
    input: SettingsInput,
) -> CmdResult<SettingsView> {
    let mut s = state.settings.lock().unwrap();
    let mut next = s.clone();
    next.apply(input);
    next.save(&state.settings_path)?;
    *s = next;
    let view = SettingsView::from(&*s);
    #[cfg(desktop)]
    let close_to_tray = s.close_action == CloseAction::MinimizeToTray;
    drop(s);

    // The tray icon only exists while closing to the tray is enabled. Menu and
    // tray creation must happen on the main thread.
    #[cfg(desktop)]
    {
        let handle = app.clone();
        let _ = app.run_on_main_thread(move || {
            if let Err(e) = set_tray_enabled(&handle, close_to_tray) {
                eprintln!("failed to update the tray icon: {e}");
            }
        });
    }

    Ok(view)
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
            let settings = Settings::load(&settings_path);
            #[cfg(desktop)]
            let close_to_tray = settings.close_action == CloseAction::MinimizeToTray;
            app.manage(AppState {
                db: Mutex::new(db),
                settings: Mutex::new(settings),
                settings_path,
                syncing: AtomicBool::new(false),
                hidden_to_tray: AtomicBool::new(false),
            });

            // Only show a tray icon when closing the window hides the app there.
            #[cfg(desktop)]
            if let Err(e) = set_tray_enabled(app.handle(), close_to_tray) {
                eprintln!("failed to create the tray icon: {e}");
            }

            // The window is created hidden and shown by the frontend as soon as
            // it runs. If the frontend never gets that far (script error, etc.),
            // don't leave the user with an invisible app.
            if let Some(window) = app.get_webview_window("main") {
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_secs(2));
                    // Unless the user already closed it to the tray, in which
                    // case popping it back up would be unwanted.
                    if !window
                        .state::<AppState>()
                        .hidden_to_tray
                        .load(Ordering::SeqCst)
                    {
                        let _ = window.show();
                    }
                });
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let minimize = window.state::<AppState>().settings.lock().unwrap().close_action
                    == CloseAction::MinimizeToTray;
                if minimize {
                    api.prevent_close();
                    #[cfg(desktop)]
                    {
                        window
                            .state::<AppState>()
                            .hidden_to_tray
                            .store(true, Ordering::SeqCst);
                        let tray = window
                            .app_handle()
                            .tray_by_id(&tauri::tray::TrayIconId::new(TRAY_ID))
                            .is_some();
                        if tray {
                            // Hiding (rather than minimizing) drops the taskbar
                            // button, so the app lives in the tray only.
                            let _ = window.hide();
                        } else {
                            // No tray available (e.g. no appindicator on Linux):
                            // fall back to the taskbar so the window is reachable.
                            let _ = window.minimize();
                        }
                    }
                    #[cfg(not(desktop))]
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
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app, _event| {
            // Clicking the Dock icon on macOS should restore a window that was
            // closed to the tray.
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Reopen { .. } = _event {
                show_main_window(_app);
            }
        });
}
