#![cfg_attr(not(target_os = "macos"), allow(dead_code, unused_imports))]

#[cfg(not(target_os = "macos"))]
compile_error!("VibeInspector tray is supported on macOS only");

use std::{
    fs,
    sync::{Arc, RwLock},
    thread,
};

use tao::{
    event::Event,
    event_loop::{ControlFlow, EventLoopBuilder},
    platform::macos::{ActivationPolicy, EventLoopExtMacOS},
};
use tokio_util::sync::CancellationToken;
use tray_icon::{
    menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem},
    Icon, TrayIconBuilder,
};
use vibe_inspector_lib::{
    storage::InspectorDatabase,
    tray_runtime::{
        bind_mcp_listener, claim_control_socket, clear_quit_intent, configured_mcp_bind,
        install_local_panic_hook, is_login_item_enabled, load_or_create_bearer_token,
        mark_quit_intent, open_desktop, record_diagnostic, serve_control_socket, serve_mcp_http,
        set_login_item_enabled, ControlAction, DiagnosticEvent, DiagnosticProcess, SocketClaim,
        TrayError, TrayHealth, TrayPaths,
    },
};

#[derive(Debug)]
enum UserEvent {
    Menu(MenuEvent),
    Control(ControlAction),
    McpOnline(String),
    McpError(String),
    RuntimeStopped,
}

fn main() {
    if let Err(error) = run_tray() {
        eprintln!("VibeInspector tray failed: {error}");
        std::process::exit(1);
    }
}

fn run_tray() -> Result<(), TrayError> {
    let paths = TrayPaths::for_macos_user()?;
    install_local_panic_hook(paths.clone(), DiagnosticProcess::Tray);
    let listener = match claim_control_socket(&paths)? {
        SocketClaim::AlreadyRunning => return Ok(()),
        SocketClaim::Claimed(listener) => listener,
    };
    // Any tray start is an explicit activation (desktop launch, login item, or
    // manual start), so a previously recorded quit intent no longer applies.
    let _ = clear_quit_intent(&paths);
    let bearer_token = load_or_create_bearer_token(&paths)?;
    let database = InspectorDatabase::for_macos_user()
        .map_err(|error| TrayError::Runtime(error.to_string()))?;
    database
        .initialize()
        .map_err(|error| TrayError::Runtime(error.to_string()))?;

    let login_enabled = is_login_item_enabled(&paths);
    let health = Arc::new(RwLock::new(TrayHealth::starting(login_enabled)));
    let cancellation = CancellationToken::new();
    let _ = record_diagnostic(&paths, DiagnosticProcess::Tray, DiagnosticEvent::Started);

    let mut event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    event_loop.set_activation_policy(ActivationPolicy::Accessory);
    event_loop.set_dock_visibility(false);
    let proxy = event_loop.create_proxy();

    MenuEvent::set_event_handler(Some({
        let proxy = proxy.clone();
        move |event| {
            let _ = proxy.send_event(UserEvent::Menu(event));
        }
    }));

    let status_item = MenuItem::with_id("status", "MCP: Starting...", false, None);
    let endpoint_item = MenuItem::with_id("endpoint", "Local endpoint is starting", false, None);
    let open_item = MenuItem::with_id("open", "Open VibeInspector", true, None);
    let login_item = CheckMenuItem::with_id("login", "Start at Login", true, login_enabled, None);
    let quit_item = MenuItem::with_id("quit", "Quit VibeInspector Services", true, None);
    let separator_one = PredefinedMenuItem::separator();
    let separator_two = PredefinedMenuItem::separator();
    let menu = Menu::with_items(&[
        &status_item,
        &endpoint_item,
        &separator_one,
        &open_item,
        &login_item,
        &separator_two,
        &quit_item,
    ])
    .map_err(|error| TrayError::Runtime(error.to_string()))?;

    let _tray_icon = TrayIconBuilder::new()
        .with_id("vibe-inspector")
        .with_menu(Box::new(menu))
        .with_menu_on_left_click(true)
        .with_menu_on_right_click(true)
        .with_tooltip("VibeInspector services")
        .with_icon(template_icon()?)
        .with_icon_as_template(true)
        .build()
        .map_err(|error| TrayError::Runtime(error.to_string()))?;

    start_runtime_thread(
        listener,
        database,
        bearer_token,
        paths.clone(),
        health.clone(),
        cancellation.clone(),
        proxy.clone(),
    );

    let mut quitting = false;
    event_loop.run(move |event, _target, control_flow| {
        *control_flow = ControlFlow::Wait;
        match event {
            Event::UserEvent(UserEvent::Menu(event)) if event.id() == open_item.id() => {
                let _ = open_desktop();
            }
            Event::UserEvent(UserEvent::Menu(event)) if event.id() == login_item.id() => {
                let enabled = login_item.is_checked();
                let executable = std::env::current_exe();
                let result = executable
                    .map_err(TrayError::from)
                    .and_then(|executable| set_login_item_enabled(&paths, &executable, enabled));
                if result.is_err() {
                    login_item.set_checked(!enabled);
                }
                if let Ok(mut health) = health.write() {
                    health.login_item_enabled = login_item.is_checked();
                }
            }
            Event::UserEvent(UserEvent::Menu(event)) if event.id() == quit_item.id() => {
                begin_shutdown(
                    &mut quitting,
                    &cancellation,
                    &status_item,
                    &open_item,
                    &login_item,
                    &quit_item,
                );
            }
            Event::UserEvent(UserEvent::Control(ControlAction::OpenDesktop)) => {
                let _ = open_desktop();
            }
            Event::UserEvent(UserEvent::Control(ControlAction::LoginItemChanged(enabled))) => {
                login_item.set_checked(enabled);
            }
            Event::UserEvent(UserEvent::Control(ControlAction::Quit)) => {
                begin_shutdown(
                    &mut quitting,
                    &cancellation,
                    &status_item,
                    &open_item,
                    &login_item,
                    &quit_item,
                );
            }
            Event::UserEvent(UserEvent::McpOnline(endpoint)) => {
                status_item.set_text("MCP: Online");
                endpoint_item.set_text(endpoint);
                let _ =
                    record_diagnostic(&paths, DiagnosticProcess::Tray, DiagnosticEvent::McpOnline);
            }
            Event::UserEvent(UserEvent::McpError(error)) => {
                status_item.set_text("MCP: Unavailable");
                endpoint_item.set_text(compact_error(&error));
                let _ = record_diagnostic(
                    &paths,
                    DiagnosticProcess::Tray,
                    DiagnosticEvent::McpUnavailable,
                );
            }
            Event::UserEvent(UserEvent::RuntimeStopped) => {
                *control_flow = ControlFlow::Exit;
            }
            Event::LoopDestroyed => {
                cancellation.cancel();
            }
            _ => {}
        }
    })
}

fn start_runtime_thread(
    control_listener: std::os::unix::net::UnixListener,
    database: InspectorDatabase,
    bearer_token: String,
    paths: TrayPaths,
    health: Arc<RwLock<TrayHealth>>,
    cancellation: CancellationToken,
    proxy: tao::event_loop::EventLoopProxy<UserEvent>,
) {
    thread::spawn(move || {
        let runtime = match tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .worker_threads(2)
            .thread_name("vibe-inspector-tray")
            .build()
        {
            Ok(runtime) => runtime,
            Err(error) => {
                let _ = proxy.send_event(UserEvent::McpError(error.to_string()));
                let _ = proxy.send_event(UserEvent::RuntimeStopped);
                return;
            }
        };

        runtime.block_on(async {
            let action = Arc::new({
                let proxy = proxy.clone();
                move |action| {
                    let _ = proxy.send_event(UserEvent::Control(action));
                }
            });
            let control_task = tokio::spawn({
                let cancellation = cancellation.child_token();
                let health = health.clone();
                async move {
                    serve_control_socket(control_listener, health, action, cancellation).await
                }
            });

            let http_task = match configured_mcp_bind() {
                Ok(address) => match bind_mcp_listener(address).await {
                    Ok(listener) => {
                        let endpoint = listener
                            .local_addr()
                            .map(|address| format!("http://{address}/mcp"))
                            .unwrap_or_else(|_| format!("http://{address}/mcp"));
                        if let Ok(mut health) = health.write() {
                            health.mark_online(endpoint.clone());
                        }
                        let _ = proxy.send_event(UserEvent::McpOnline(endpoint));
                        Some(tokio::spawn({
                            let cancellation = cancellation.child_token();
                            let paths = paths.clone();
                            let proxy = proxy.clone();
                            let health = health.clone();
                            async move {
                                if let Err(error) = serve_mcp_http(
                                    listener,
                                    database,
                                    bearer_token,
                                    paths,
                                    cancellation,
                                )
                                .await
                                {
                                    if let Ok(mut health) = health.write() {
                                        health.mark_error(error.to_string());
                                    }
                                    let _ = proxy.send_event(UserEvent::McpError(error.to_string()));
                                }
                            }
                        }))
                    }
                    Err(error) => {
                        if let Ok(mut health) = health.write() {
                            health.mark_error(error.to_string());
                        }
                        let _ = proxy.send_event(UserEvent::McpError(error.to_string()));
                        None
                    }
                },
                Err(error) => {
                    if let Ok(mut health) = health.write() {
                        health.mark_error(error.to_string());
                    }
                    let _ = proxy.send_event(UserEvent::McpError(error.to_string()));
                    None
                }
            };

            cancellation.cancelled().await;
            if let Some(http_task) = http_task {
                let _ = http_task.await;
            }
            let _ = control_task.await;
        });

        let _ = fs::remove_file(&paths.socket_path);
        let _ = fs::remove_file(&paths.endpoint_path);
        let _ = proxy.send_event(UserEvent::RuntimeStopped);
    });
}

fn begin_shutdown(
    quitting: &mut bool,
    cancellation: &CancellationToken,
    status_item: &MenuItem,
    open_item: &MenuItem,
    login_item: &CheckMenuItem,
    quit_item: &MenuItem,
) {
    if *quitting {
        return;
    }
    *quitting = true;
    if let Ok(paths) = TrayPaths::for_macos_user() {
        let _ = mark_quit_intent(&paths);
        let _ = record_diagnostic(&paths, DiagnosticProcess::Tray, DiagnosticEvent::Shutdown);
    }
    status_item.set_text("MCP: Stopping...");
    open_item.set_enabled(false);
    login_item.set_enabled(false);
    quit_item.set_enabled(false);
    cancellation.cancel();
}

fn compact_error(error: &str) -> String {
    let first_line = error.lines().next().unwrap_or("MCP runtime error");
    let mut characters = first_line.chars();
    let compact = characters.by_ref().take(72).collect::<String>();
    if characters.next().is_some() {
        format!("{compact}...")
    } else {
        compact
    }
}

fn template_icon() -> Result<Icon, TrayError> {
    let rgba = template_icon_rgba();
    Icon::from_rgba(rgba, TEMPLATE_ICON_SIZE, TEMPLATE_ICON_SIZE)
        .map_err(|error| TrayError::Runtime(error.to_string()))
}

const TEMPLATE_ICON_SIZE: u32 = 36;
const TEMPLATE_ICON_SUPERSAMPLING: u32 = 4;

fn template_icon_rgba() -> Vec<u8> {
    let mut rgba = vec![0_u8; (TEMPLATE_ICON_SIZE * TEMPLATE_ICON_SIZE * 4) as usize];
    let sample_count = TEMPLATE_ICON_SUPERSAMPLING * TEMPLATE_ICON_SUPERSAMPLING;

    // tray-icon renders this 2x bitmap at 18pt. The 2px transparent margin keeps the
    // visible mark close to the 16pt optical size used by neighboring macOS status items.
    for y in 0..TEMPLATE_ICON_SIZE {
        for x in 0..TEMPLATE_ICON_SIZE {
            let mut covered_samples = 0_u32;
            for sample_y in 0..TEMPLATE_ICON_SUPERSAMPLING {
                for sample_x in 0..TEMPLATE_ICON_SUPERSAMPLING {
                    let sample_x =
                        x as f32 + (sample_x as f32 + 0.5) / TEMPLATE_ICON_SUPERSAMPLING as f32;
                    let sample_y =
                        y as f32 + (sample_y as f32 + 0.5) / TEMPLATE_ICON_SUPERSAMPLING as f32;
                    if template_icon_contains(sample_x, sample_y) {
                        covered_samples += 1;
                    }
                }
            }

            let alpha = ((covered_samples * 255 + sample_count / 2) / sample_count) as u8;
            let index = ((y * TEMPLATE_ICON_SIZE + x) * 4) as usize;
            rgba[index + 3] = alpha;
        }
    }

    rgba
}

fn template_icon_contains(x: f32, y: f32) -> bool {
    let lens_distance = ((x - 19.25).powi(2) + (y - 13.0).powi(2)).sqrt();
    let lens = (9.0..=11.35).contains(&lens_distance);
    let handle = distance_to_segment(x, y, 13.45, 22.0, 6.6, 31.65) <= 2.15;

    let flow_line = distance_to_segment(x, y, 12.4, 14.0, 26.2, 14.0) <= 0.62;
    let left_node = point_in_square(x, y, 13.2, 14.0, 1.6);
    let center_node = point_in_square(x, y, 19.3, 14.0, 1.6);
    let right_node = point_in_square(x, y, 25.4, 14.0, 1.6);
    let signal_dot = (x - 17.0).powi(2) + (y - 10.25).powi(2) <= 1.25_f32.powi(2);

    lens || handle || flow_line || left_node || center_node || right_node || signal_dot
}

fn point_in_square(x: f32, y: f32, center_x: f32, center_y: f32, half_size: f32) -> bool {
    (x - center_x).abs() <= half_size && (y - center_y).abs() <= half_size
}

fn distance_to_segment(x: f32, y: f32, start_x: f32, start_y: f32, end_x: f32, end_y: f32) -> f32 {
    let segment_x = end_x - start_x;
    let segment_y = end_y - start_y;
    let length_squared = segment_x * segment_x + segment_y * segment_y;
    let projection =
        (((x - start_x) * segment_x + (y - start_y) * segment_y) / length_squared).clamp(0.0, 1.0);
    let closest_x = start_x + projection * segment_x;
    let closest_y = start_y + projection * segment_y;
    ((x - closest_x).powi(2) + (y - closest_y).powi(2)).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_icon_matches_the_native_menu_bar_footprint() {
        let rgba = template_icon_rgba();
        let mut min_x = TEMPLATE_ICON_SIZE;
        let mut min_y = TEMPLATE_ICON_SIZE;
        let mut max_x = 0;
        let mut max_y = 0;

        for y in 0..TEMPLATE_ICON_SIZE {
            for x in 0..TEMPLATE_ICON_SIZE {
                let alpha = rgba[((y * TEMPLATE_ICON_SIZE + x) * 4 + 3) as usize];
                if alpha > 0 {
                    min_x = min_x.min(x);
                    min_y = min_y.min(y);
                    max_x = max_x.max(x);
                    max_y = max_y.max(y);
                }
            }
        }

        assert_eq!(
            rgba.len(),
            (TEMPLATE_ICON_SIZE * TEMPLATE_ICON_SIZE * 4) as usize
        );
        assert_eq!((min_x, min_y, max_x, max_y), (4, 1, 30, 33));
    }
}
