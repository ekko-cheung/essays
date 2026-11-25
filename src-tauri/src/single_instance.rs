use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions, TryLockError};
use std::io;
use std::net::{Ipv4Addr, UdpSocket};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager, RunEvent, Wry, async_runtime, plugin};

const OPEN_WINDOW: &[u8] = b"\0essays:open-window";
const RETRY_INTERVAL: Duration = Duration::from_millis(100);
const STARTUP_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Serialize, Deserialize)]
struct Endpoint {
    pid: u32,
    port: u16,
}

struct PrimaryInstance {
    _lock: File,
    endpoint_path: PathBuf,
    socket: Option<UdpSocket>,
    listener: Option<async_runtime::JoinHandle<()>>,
}

impl Drop for PrimaryInstance {
    fn drop(&mut self) {
        if let Some(listener) = self.listener.take() {
            listener.abort();
        }
        // Remove the address before releasing the lock to the next instance.
        let _ = fs::remove_file(&self.endpoint_path);
    }
}

pub fn init() -> plugin::TauriPlugin<Wry> {
    plugin::Builder::new("single-instance")
        .setup(|app, _| {
            if tauri::is_dev() {
                return Ok(());
            }

            let dir = app.path().app_data_dir()?;
            fs::create_dir_all(&dir)?;
            let lock = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(false)
                .open(dir.join("instance.lock"))?;
            let endpoint_path = dir.join("instance.endpoint");
            let deadline = Instant::now() + STARTUP_TIMEOUT;
            loop {
                match lock.try_lock() {
                    Ok(()) => break,
                    Err(TryLockError::WouldBlock) => {
                        if notify_primary(&endpoint_path).is_ok() {
                            std::process::exit(0);
                        }
                        if Instant::now() >= deadline {
                            eprintln!("timed out contacting the running Essays instance");
                            std::process::exit(0);
                        }
                        // The primary may still be starting, or exiting during an update.
                        std::thread::sleep(RETRY_INTERVAL);
                    }
                    Err(TryLockError::Error(error)) => return Err(error.into()),
                }
            }

            let socket = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0))?;
            socket.set_nonblocking(true)?;
            let endpoint = Endpoint {
                pid: std::process::id(),
                port: socket.local_addr()?.port(),
            };
            fs::write(&endpoint_path, serde_json::to_vec(&endpoint)?)?;
            app.manage(Mutex::new(Some(PrimaryInstance {
                _lock: lock,
                endpoint_path,
                socket: Some(socket),
                listener: None,
            })));
            Ok(())
        })
        .on_event(|app, event| {
            let Some(state) = app.try_state::<Mutex<Option<PrimaryInstance>>>() else {
                return;
            };
            match event {
                RunEvent::Ready => {
                    // Wait for the application state and its initial windows to be ready.
                    if let Some(instance) = state.lock().unwrap().as_mut() {
                        let socket = instance.socket.take().expect("instance socket is ready");
                        let app = app.clone();
                        instance.listener = Some(async_runtime::spawn(async move {
                            if let Err(error) = listen(app, socket).await {
                                eprintln!("failed to listen for Essays activation: {error}");
                            }
                        }));
                    }
                }
                RunEvent::Exit => {
                    // Tauri can spawn its replacement before the current process exits.
                    state.lock().unwrap().take();
                }
                _ => {}
            }
        })
        .build()
}

fn notify_primary(path: &Path) -> io::Result<()> {
    let endpoint: Endpoint = serde_json::from_slice(&fs::read(path)?)?;
    let socket = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0))?;
    socket.connect((Ipv4Addr::LOCALHOST, endpoint.port))?;
    socket.set_read_timeout(Some(RETRY_INTERVAL))?;

    // Windows gives a newly launched process the right to activate a window.
    // Pass it to the process that owns the window before requesting activation.
    #[cfg(windows)]
    if endpoint.pid != 0 && endpoint.pid != u32::MAX {
        unsafe {
            windows_sys::Win32::UI::WindowsAndMessaging::AllowSetForegroundWindow(endpoint.pid);
        }
    }

    socket.send(OPEN_WINDOW)?;
    let mut reply = [0; OPEN_WINDOW.len() + 1];
    let len = socket.recv(&mut reply)?;
    if &reply[..len] != OPEN_WINDOW {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid activation reply",
        ));
    }
    Ok(())
}

async fn listen(app: AppHandle, socket: UdpSocket) -> io::Result<()> {
    let socket = tokio::net::UdpSocket::from_std(socket)?;
    let mut request = vec![0; 65_536];
    loop {
        let (len, sender) = match socket.recv_from(&mut request).await {
            Ok(packet) => packet,
            // A launcher can close its socket before a delayed reply reaches it.
            Err(error) if error.kind() == io::ErrorKind::ConnectionReset => continue,
            Err(error) => return Err(error),
        };
        if &request[..len] == OPEN_WINDOW {
            let handle = app.clone();
            if app
                .run_on_main_thread(move || crate::open_from_tray(&handle))
                .is_ok()
            {
                let _ = socket.send_to(OPEN_WINDOW, sender).await;
            }
        }
    }
}
