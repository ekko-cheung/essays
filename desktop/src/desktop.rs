use crate::{
    service::{APP_ID, Result, Service},
    settings::CloseAction,
    single_instance::PrimaryInstance,
    workspace::{self, Workspace},
};
use gpui_kit::{
    component::{ActiveTheme, StyledExt, button::Button},
    *,
};
use std::{sync::Arc, time::Duration};
use tokio::{runtime::Handle, sync::mpsc};

pub enum Command {
    Open,
    Quit,
}

pub struct Desktop {
    service: Arc<Service>,
    runtime: Handle,
    window: Option<(AnyWindowHandle, Entity<Workspace>)>,
    #[cfg(any(windows, target_os = "macos"))]
    tray: Option<tray_icon::TrayIcon>,
}

impl Global for Desktop {}

pub fn run() {
    let startup = Service::data_dir().and_then(|dir| {
        let instance = PrimaryInstance::acquire(&dir)
            .map_err(|error| format!("Could not use {}. {error}", dir.display()))?;
        instance
            .map(|instance| Service::open(&dir).map(|service| (instance, Arc::new(service))))
            .transpose()
    });
    let startup = match startup {
        Ok(None) => return,
        Ok(Some(startup)) => tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .map(|runtime| (startup, runtime))
            .map_err(|e| e.to_string()),
        Err(error) => Err(error),
    };
    let app = gpui_kit::application()
        .with_assets(gpui_kit::assets::AllAssets)
        .with_quit_mode(QuitMode::Explicit);
    app.on_reopen(open);
    app.run(move |cx| {
        gpui_kit::init(cx);
        cx.set_app_identity(APP_ID, "Essays");
        let ((mut instance, service), runtime) = match startup {
            Ok(startup) => startup,
            Err(error) => return startup_error(error, cx),
        };
        let (commands, mut events) = mpsc::unbounded_channel();
        #[cfg(any(windows, target_os = "macos"))]
        bind_tray_events(&commands);
        cx.set_global(Desktop {
            service,
            runtime: runtime.handle().clone(),
            window: None,
            #[cfg(any(windows, target_os = "macos"))]
            tray: None,
        });
        cx.set_global(workspace::UpdateSession { checked: false });
        workspace::bind_keys(cx);
        cx.on_action(|_: &workspace::Quit, cx| quit(cx));
        cx.set_menus([Menu {
            name: "Essays".into(),
            disabled: false,
            items: vec![
                MenuItem::action("Settings…", workspace::OpenSettings),
                MenuItem::separator(),
                MenuItem::action("Quit Essays", workspace::Quit),
            ],
        }]);
        open(cx);
        instance.listen(runtime.handle(), move || {
            commands.send(Command::Open).is_ok()
        });
        cx.spawn(async move |cx| {
            // Keep the lock and runtime alive while Essays is running, including in the tray.
            let _instance = instance;
            let _runtime = RuntimeOwner(Some(runtime));
            while let Some(event) = events.recv().await {
                cx.update(|cx| match event {
                    Command::Open => open(cx),
                    Command::Quit => quit(cx),
                });
            }
        })
        .detach();
        start_auto_sync(cx);
    });
}

// Dropping a Tokio runtime on a thread that is driving another async executor
// can block on outstanding work. Shutdown on a separate thread if necessary.
struct RuntimeOwner(Option<tokio::runtime::Runtime>);

impl Drop for RuntimeOwner {
    fn drop(&mut self) {
        if let Some(runtime) = self.0.take() {
            runtime.shutdown_background();
        }
    }
}

fn startup_error(error: String, cx: &mut App) {
    eprintln!("{error}");
    if gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
        window.set_window_title("Essays");
        window.on_window_should_close(cx, |_, cx| {
            cx.quit();
            true
        });
        cx.new(|_| StartupError(error.into()))
    })
    .is_err()
    {
        cx.quit();
    }
}

struct StartupError(SharedString);

impl Render for StartupError {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .v_flex()
            .size_full()
            .p_6()
            .gap_4()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(div().text_lg().child("Couldn’t open Essays"))
            .child(self.0.clone())
            .child(
                Button::new("quit")
                    .label("Quit")
                    .on_click(|_, _, cx| cx.quit()),
            )
    }
}

fn open(cx: &mut App) {
    let Some(desktop) = cx.try_global::<Desktop>() else {
        return;
    };
    let service = desktop.service.clone();
    let runtime = desktop.runtime.clone();
    if let Some((handle, _)) = desktop.window.clone()
        && handle
            .update(cx, |_, window, cx| {
                cx.activate(true);
                window.activate_window();
            })
            .is_ok()
    {
        return;
    }
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
            None,
            size(px(1100.), px(720.)),
            cx,
        ))),
        window_min_size: Some(size(px(780.), px(400.))),
        ..Default::default()
    };
    match gpui_kit::open_window(options, cx, |window, cx| {
        window.set_window_title("Essays");
        let workspace = cx.new(|cx| Workspace::new(service, runtime, window, cx));
        let view = workspace.downgrade();
        window.on_window_should_close(cx, move |_, cx| {
            if !view
                .update(cx, |workspace, cx| workspace.flush(cx))
                .unwrap_or(true)
            {
                return false;
            }
            cx.defer(close);
            false
        });
        workspace
    }) {
        Ok(window) => {
            let workspace = window.1.clone();
            cx.global_mut::<Desktop>().window = Some(window);
            if let Err(error) = refresh_tray(cx) {
                workspace.update(cx, |view, cx| view.report(error, cx));
            }
            cx.activate(true);
        }
        Err(error) => startup_error(error.to_string(), cx),
    }
}

fn close(cx: &mut App) {
    let desktop = cx.global::<Desktop>();
    if !tray_ready(cx) || desktop.service.settings().close_action == CloseAction::Quit {
        cx.quit();
        return;
    }
    if let Some((window, _)) = cx.global_mut::<Desktop>().window.take() {
        let _ = window.update(cx, |_, window, _| window.remove_window());
    }
}

fn quit(cx: &mut App) {
    let workspace = cx
        .try_global::<Desktop>()
        .and_then(|d| d.window.as_ref().map(|(_, view)| view.clone()));
    if workspace.is_none_or(|view| view.update(cx, |view, cx| view.flush(cx))) {
        cx.quit();
    }
}

pub fn refresh_open_workspace(cx: &mut App) {
    if let Some((handle, view)) = cx.global::<Desktop>().window.clone() {
        let _ = handle.update(cx, |_, window, cx| {
            view.update(cx, |view, cx| view.refresh_after_sync(window, cx));
        });
    }
}

fn start_auto_sync(cx: &mut App) {
    let service = cx.global::<Desktop>().service.clone();
    let runtime = cx.global::<Desktop>().runtime.clone();
    let timer = cx.background_executor().clone();
    cx.spawn(async move |cx| {
        use std::time::Instant;
        let mut schedule = None;
        loop {
            let settings = service.settings().sync;
            let interval =
                Duration::from_secs(u64::from(settings.interval_minutes.clamp(1, 1440)) * 60);
            let config = (settings.account_id, settings.database_id, interval);
            if settings.enabled && settings.auto_sync && !settings.api_token.is_empty() {
                let (previous, deadline) =
                    schedule.get_or_insert_with(|| (config.clone(), Instant::now() + interval));
                if *previous != config {
                    *previous = config;
                    *deadline = Instant::now() + interval;
                } else if Instant::now() >= *deadline {
                    *deadline = Instant::now() + interval;
                    let service = service.clone();
                    let result = runtime.spawn(async move { service.sync().await }).await;
                    cx.update(refresh_open_workspace);
                    if let Ok(Err(error)) = result {
                        eprintln!("Background sync failed: {error}");
                    }
                }
            } else {
                schedule = None;
            }
            timer.timer(Duration::from_secs(1)).await;
        }
    })
    .detach();
}

#[cfg(any(windows, target_os = "macos"))]
fn tray_ready(cx: &App) -> bool {
    cx.global::<Desktop>().tray.is_some()
}

#[cfg(not(any(windows, target_os = "macos")))]
fn tray_ready(_: &App) -> bool {
    false
}

#[cfg(not(any(windows, target_os = "macos")))]
pub fn refresh_tray(_: &mut App) -> Result<()> {
    Ok(())
}

#[cfg(any(windows, target_os = "macos"))]
fn bind_tray_events(commands: &mpsc::UnboundedSender<Command>) {
    use tray_icon::{MouseButton, MouseButtonState, TrayIconEvent, menu::MenuEvent};
    let menu_commands = commands.clone();
    MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
        let command = match event.id.as_ref() {
            "essays-tray-open" => Command::Open,
            "essays-tray-quit" => Command::Quit,
            _ => return,
        };
        let _ = menu_commands.send(command);
    }));
    let click_commands = commands.clone();
    TrayIconEvent::set_event_handler(Some(move |event: TrayIconEvent| {
        if let TrayIconEvent::Click {
            button: MouseButton::Left,
            button_state: MouseButtonState::Up,
            ..
        } = event
            && cfg!(windows)
        {
            let _ = click_commands.send(Command::Open);
        }
    }));
}

#[cfg(any(windows, target_os = "macos"))]
pub fn refresh_tray(cx: &mut App) -> Result<()> {
    use tray_icon::{
        Icon, TrayIconBuilder,
        menu::{Menu, MenuItem},
    };
    let desktop = cx.global_mut::<Desktop>();
    if desktop.service.settings().close_action == CloseAction::Quit {
        desktop.tray = None;
        return Ok(());
    }
    if desktop.tray.is_some() {
        return Ok(());
    }
    let menu = Menu::new();
    let open = MenuItem::with_id("essays-tray-open", "Open Essays", true, None);
    let quit = MenuItem::with_id("essays-tray-quit", "Quit Essays", true, None);
    menu.append_items(&[&open, &quit])
        .map_err(|e| e.to_string())?;
    let image = image::load_from_memory(include_bytes!("../assets/32x32.png"))
        .map_err(|e| e.to_string())?
        .into_rgba8();
    let (width, height) = image.dimensions();
    let icon = Icon::from_rgba(image.into_raw(), width, height).map_err(|e| e.to_string())?;
    desktop.tray = Some(
        TrayIconBuilder::new()
            .with_tooltip("Essays")
            .with_icon(icon)
            .with_menu(Box::new(menu))
            .with_menu_on_left_click(cfg!(target_os = "macos"))
            .build()
            .map_err(|e| e.to_string())?,
    );
    Ok(())
}
