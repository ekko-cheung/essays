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
#[cfg(target_os = "macos")]
mod macos_tray;
mod settings;
#[cfg(desktop)]
mod single_instance;

struct AppState {
    db: Mutex<Db>,
    settings: Mutex<Settings>,
    settings_path: PathBuf,
    syncing: AtomicBool,
    /// Prevent the startup visibility fallback from undoing a minimize.
    minimized_on_close: AtomicBool,
    #[cfg(desktop)]
    tray_sync: Mutex<Option<tauri::async_runtime::JoinHandle<()>>>,
    #[cfg(all(desktop, not(target_os = "macos")))]
    reopen_window: Mutex<()>,
}

impl AppState {
    fn closes_to_tray(&self) -> bool {
        self.settings.lock().unwrap().close_action == CloseAction::MinimizeToTray
    }
}

fn initialize_state(app: &tauri::AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let dir = app.path().app_data_dir()?;
    std::fs::create_dir_all(&dir)?;
    let db = Db::open(dir.join("essays.db"))?;
    let settings_path = dir.join("settings.json");
    let settings = Settings::load(&settings_path);
    app.manage(AppState {
        db: Mutex::new(db),
        settings: Mutex::new(settings),
        settings_path,
        syncing: AtomicBool::new(false),
        minimized_on_close: AtomicBool::new(false),
        #[cfg(desktop)]
        tray_sync: Mutex::new(None),
        #[cfg(all(desktop, not(target_os = "macos")))]
        reopen_window: Mutex::new(()),
    });
    Ok(())
}

type CmdResult<T> = Result<T, String>;

fn err(e: essays_core::Error) -> String {
    e.to_string()
}

const MAIN_WINDOW_LABEL: &str = "main";
#[cfg(desktop)]
const TRAY_ID: &str = "essays-tray";

#[cfg(desktop)]
fn focus_main_window(app: &tauri::AppHandle) -> bool {
    let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) else {
        return false;
    };
    let _ = window.unminimize();
    let _ = window.show();
    let _ = window.set_focus();
    true
}

#[cfg(target_os = "macos")]
use macos_tray::open_window as open_from_tray;

#[cfg(all(desktop, not(target_os = "macos")))]
fn open_from_tray(app: &tauri::AppHandle) {
    if focus_main_window(app) {
        return;
    }
    let _ = stop_tray_sync(app);

    // A native close can destroy the window. Rebuild it off the event handler
    // thread so WebView2 can initialize on Windows.
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let _guard = state.reopen_window.lock().unwrap();
        if focus_main_window(&app) {
            return;
        }

        if let Some(config) = app
            .config()
            .app
            .windows
            .iter()
            .find(|config| config.label == MAIN_WINDOW_LABEL)
        {
            match tauri::WebviewWindowBuilder::from_config(&app, config)
                .and_then(|builder| builder.visible(true).build())
            {
                Ok(window) => {
                    let _ = window.set_focus();
                }
                Err(e) => eprintln!("failed to reopen the main window: {e}"),
            }
        }
    });
}

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
            "open" => open_from_tray(app),
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
                open_from_tray(tray.app_handle());
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
fn set_note_pinned(state: State<AppState>, id: String, pinned: bool) -> CmdResult<()> {
    state
        .db
        .lock()
        .unwrap()
        .set_note_pinned(&id, pinned)
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
    _app: tauri::AppHandle,
    state: State<AppState>,
    input: SettingsInput,
) -> CmdResult<SettingsView> {
    let mut s = state.settings.lock().unwrap();
    let mut next = s.clone();
    next.apply(input);
    next.save(&state.settings_path)?;
    *s = next;
    let view = SettingsView::from(&*s);
    #[cfg(all(desktop, not(target_os = "macos")))]
    let close_to_tray = s.close_action == CloseAction::MinimizeToTray;
    drop(s);

    #[cfg(target_os = "macos")]
    macos_tray::notify_settings_changed();
    #[cfg(all(desktop, not(target_os = "macos")))]
    {
        let handle = _app.clone();
        let _ = _app.run_on_main_thread(move || {
            if let Err(e) = set_tray_enabled(&handle, close_to_tray) {
                eprintln!("failed to update the tray icon: {e}");
            }
        });
    }

    Ok(view)
}

fn minimize_on_close(_window: &tauri::Window) -> bool {
    #[cfg(target_os = "macos")]
    return false;
    #[cfg(all(desktop, not(target_os = "macos")))]
    return _window.state::<AppState>().closes_to_tray()
        && _window.app_handle().tray_by_id(TRAY_ID).is_none();
    #[cfg(mobile)]
    return _window.state::<AppState>().closes_to_tray();
}

#[tauri::command]
fn close_window(window: tauri::Window) -> CmdResult<()> {
    if minimize_on_close(&window) {
        window.minimize()
    } else {
        window.destroy()
    }
    .map_err(|e| e.to_string())
}

#[tauri::command]
fn restart_after_update(app: tauri::AppHandle) {
    #[cfg(target_os = "macos")]
    app.exit(macos_tray::RESTART_PARENT);
    #[cfg(not(target_os = "macos"))]
    app.request_restart();
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

async fn sync(state: &AppState) -> CmdResult<cloud::SyncReport> {
    let cfg = state.settings.lock().unwrap().sync.clone();
    if !cfg.enabled {
        return Err("Sync is turned off.".into());
    }
    if state.syncing.swap(true, Ordering::SeqCst) {
        return Err("A sync is already running.".into());
    }
    struct SyncGuard<'a>(&'a AtomicBool);
    impl Drop for SyncGuard<'_> {
        fn drop(&mut self) {
            self.0.store(false, Ordering::SeqCst);
        }
    }
    let _guard = SyncGuard(&state.syncing);
    cloud::run(&cfg, &state.db).await
}

#[tauri::command]
async fn sync_now(state: State<'_, AppState>) -> CmdResult<cloud::SyncReport> {
    sync(&state).await
}

#[cfg(desktop)]
fn start_tray_sync(app: &tauri::AppHandle) {
    let state = app.state::<AppState>();
    let settings = state.settings.lock().unwrap();
    let cfg = &settings.sync;
    if !cfg.enabled || !cfg.auto_sync || cfg.api_token.is_empty() {
        return;
    }

    let interval = std::time::Duration::from_secs(u64::from(cfg.interval_minutes) * 60);
    let app = app.clone();
    *state.tray_sync.lock().unwrap() = Some(tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        loop {
            tokio::time::sleep(interval).await;
            if let Err(e) = sync(&state).await {
                eprintln!("background sync failed: {e}");
            }
        }
    }));
}

#[cfg(desktop)]
fn stop_tray_sync(app: &tauri::AppHandle) -> Option<tauri::async_runtime::JoinHandle<()>> {
    let state = app.state::<AppState>();
    let task = state.tray_sync.lock().unwrap().take();
    if let Some(task) = &task {
        task.abort();
    }
    task
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let context = tauri::generate_context!();
    #[cfg(target_os = "macos")]
    if !macos_tray::is_window_process() {
        return macos_tray::run(context);
    }

    let builder = tauri::Builder::default();
    #[cfg(all(desktop, not(target_os = "macos")))]
    let builder = builder.plugin(single_instance::init());

    builder
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            initialize_state(app.handle())?;

            #[cfg(target_os = "macos")]
            macos_tray::listen_for_parent(app.handle());
            #[cfg(all(desktop, not(target_os = "macos")))]
            if let Err(e) = set_tray_enabled(app.handle(), app.state::<AppState>().closes_to_tray()) {
                eprintln!("failed to create the tray icon: {e}");
            }

            // The window is created hidden and shown by the frontend as soon as
            // it runs. If the frontend never gets that far (script error, etc.),
            // don't leave the user with an invisible app.
            if let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_secs(2));
                    if !window
                        .state::<AppState>()
                        .minimized_on_close
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
                if minimize_on_close(window) {
                    api.prevent_close();
                    window
                        .state::<AppState>()
                        .minimized_on_close
                        .store(true, Ordering::SeqCst);
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
            set_note_pinned,
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
            close_window,
            restart_after_update,
            sync_status,
            sync_test,
            sync_now
        ])
        .build(context)
        .expect("error while building tauri application")
        .run(|_app, event| match event {
            #[cfg(all(desktop, not(target_os = "macos")))]
            tauri::RunEvent::ExitRequested {
                code: None, api, ..
            } if _app.tray_by_id(TRAY_ID).is_some()
                && _app.state::<AppState>().closes_to_tray() =>
            {
                api.prevent_exit();
                start_tray_sync(_app);
            }
            #[cfg(target_os = "macos")]
            tauri::RunEvent::ExitRequested {
                code: None, api, ..
            } if _app.state::<AppState>().closes_to_tray() => {
                api.prevent_exit();
                _app.exit(macos_tray::CLOSE_TO_TRAY);
            }
            #[cfg(target_os = "macos")]
            tauri::RunEvent::Reopen { .. } => {
                focus_main_window(_app);
            }
            _ => {}
        });
}
