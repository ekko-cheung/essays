//! Keep the tray in a process that never initializes WebKit. WebKit's GPU and
//! networking processes can outlive all windows until their host process exits.

use std::io::{self, BufRead, BufReader, Write};
use std::process::{ChildStdin, Command, ExitStatus, Stdio};
use std::sync::Mutex;
use tauri::{ActivationPolicy, AppHandle, Context, Manager, RunEvent, Wry};

use crate::{
    AppState, focus_main_window, initialize_state, set_tray_enabled, start_tray_sync,
    stop_tray_sync,
};

const WINDOW_ARGUMENT: &str = "--essays-window";
const SETTINGS_CHANGED: &str = "essays:settings-changed";
const OPEN_WINDOW: &str = "essays:open-window";
const WINDOW_OPENED: &str = "essays:window-opened";
pub const CLOSE_TO_TRAY: i32 = 41;
pub const RESTART_PARENT: i32 = 42;

#[derive(Default)]
struct WindowProcess {
    input: Option<ChildStdin>,
    opening: bool,
    reopen_on_exit: bool,
}

impl WindowProcess {
    fn request_activation(&mut self) -> bool {
        let Some(input) = self.input.as_mut() else {
            return false;
        };
        if !self.reopen_on_exit {
            let _ = writeln!(input, "{OPEN_WINDOW}");
            // Reopen a child that exits to the tray before acknowledging the request.
            self.reopen_on_exit = true;
        }
        true
    }
}

pub fn is_window_process() -> bool {
    std::env::args_os().nth(1).as_deref() == Some(WINDOW_ARGUMENT.as_ref())
}

fn refresh_tray(app: &AppHandle) {
    let state = app.state::<AppState>();
    let settings = crate::settings::Settings::load(&state.settings_path);
    let enabled = settings.close_action == crate::settings::CloseAction::MinimizeToTray;
    *state.settings.lock().unwrap() = settings;

    // Keep a Dock entry as a way to open the window if the tray is unavailable.
    let policy = match set_tray_enabled(app, enabled) {
        Ok(()) => ActivationPolicy::Accessory,
        Err(e) => {
            eprintln!("failed to update the tray icon: {e}");
            ActivationPolicy::Regular
        }
    };
    let _ = app.set_activation_policy(policy);
}

pub fn run(mut context: Context<Wry>) {
    context.config_mut().app.windows.clear();
    tauri::Builder::default()
        .plugin(crate::single_instance::init())
        .setup(|app| {
            initialize_state(app.handle())?;
            app.manage(Mutex::new(WindowProcess::default()));
            refresh_tray(app.handle());
            spawn_window(app.handle(), false)?;
            Ok(())
        })
        .build(context)
        .expect("error while building Essays tray process")
        .run(|app, event| match event {
            RunEvent::Reopen { .. } => open_window(app),
            RunEvent::Exit => {
                // Closing the pipe tells the window process its parent is gone.
                app.state::<Mutex<WindowProcess>>()
                    .lock()
                    .unwrap()
                    .input
                    .take();
            }
            _ => {}
        });
}

pub fn open_window(app: &AppHandle) {
    let state = app.state::<Mutex<WindowProcess>>();
    let mut process = state.lock().unwrap();
    if process.opening || process.request_activation() {
        return;
    }
    process.opening = true;
    drop(process);

    if let Some(sync) = stop_tray_sync(app) {
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            // Wait for cancellation before the window can start its own sync.
            let _ = sync.await;
            let handle = app.clone();
            let _ = app.run_on_main_thread(move || finish_open(&handle));
        });
    } else {
        finish_open(app);
    }
}

fn finish_open(app: &AppHandle) {
    let result = spawn_window(app, true);
    app.state::<Mutex<WindowProcess>>().lock().unwrap().opening = false;
    if let Err(e) = result {
        eprintln!("failed to open Essays: {e}");
        start_tray_sync(app);
    }
}

fn spawn_window(app: &AppHandle, activate: bool) -> io::Result<()> {
    let mut child = Command::new(std::env::current_exe()?)
        .arg(WINDOW_ARGUMENT)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;
    {
        let state = app.state::<Mutex<WindowProcess>>();
        let mut process = state.lock().unwrap();
        process.input = child.stdin.take();
        if activate {
            process.request_activation();
        }
    }
    let output = BufReader::new(child.stdout.take().expect("window stdout is piped"));
    let app = app.clone();
    std::thread::spawn(move || {
        for line in output.lines().map_while(Result::ok) {
            if line == SETTINGS_CHANGED {
                let handle = app.clone();
                let _ = app.run_on_main_thread(move || refresh_tray(&handle));
            } else if line == WINDOW_OPENED {
                app.state::<Mutex<WindowProcess>>()
                    .lock()
                    .unwrap()
                    .reopen_on_exit = false;
            } else {
                println!("{line}");
            }
        }
        let status = child.wait();
        let handle = app.clone();
        let _ = app.run_on_main_thread(move || window_exited(&handle, status));
    });
    Ok(())
}

fn window_exited(app: &AppHandle, status: io::Result<ExitStatus>) {
    let state = app.state::<Mutex<WindowProcess>>();
    let mut process = state.lock().unwrap();
    process.input.take();
    let reopen = std::mem::take(&mut process.reopen_on_exit);
    drop(process);

    match status.ok().and_then(|status| status.code()) {
        Some(CLOSE_TO_TRAY) => {
            refresh_tray(app);
            if reopen {
                open_window(app);
            } else {
                start_tray_sync(app);
            }
        }
        Some(RESTART_PARENT) => app.request_restart(),
        code => app.exit(code.unwrap_or(1)),
    }
}

pub fn listen_for_parent(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        for line in io::stdin().lock().lines().map_while(Result::ok) {
            if line == OPEN_WINDOW {
                let handle = app.clone();
                let _ = app.run_on_main_thread(move || {
                    if focus_main_window(&handle) {
                        let _ = writeln!(io::stdout().lock(), "{WINDOW_OPENED}");
                    }
                });
            }
        }
        app.exit(0);
    });
}

pub fn notify_settings_changed() {
    let _ = writeln!(io::stdout().lock(), "{SETTINGS_CHANGED}");
}
