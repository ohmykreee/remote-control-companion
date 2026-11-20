mod backdrop;
mod bridge;
mod capture;
mod commands;
mod language;
mod layout;
#[cfg(target_os = "linux")]
mod linux_webview;
mod logging;
#[cfg(target_os = "linux")]
mod loopback_proxy;
mod motion;
mod paths;
mod remote;
mod settings;
mod state;
mod theme;
mod window_behavior;

use std::sync::{Arc, Mutex};

use base64::Engine;
use rand::RngCore;
use tauri::window::WindowBuilder;
use tauri::{Manager, WindowEvent};

use state::{AppState, BridgeInfo, RuntimeState};

fn random_token() -> String {
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

fn setup_error(message: impl Into<String>) -> Box<dyn std::error::Error> {
    Box::new(std::io::Error::other(message.into()))
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_window(layout::WINDOW_LABEL) {
                window_behavior::restore_hidden_window(&window);
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                        window_behavior::toggle_boss_key(app);
                    }
                })
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            commands::get_bootstrap,
            commands::save_settings,
            commands::set_window_opacity,
            commands::go_home,
            commands::open_remote,
            commands::refresh_remote,
            commands::capture_remote_backdrop,
            commands::set_chrome_overlay,
            commands::quit_app,
        ])
        .setup(|app| {
            paths::ensure_data_dirs()
                .map_err(|error| setup_error(format!("无法创建 Data 目录: {error}")))?;
            logging::info("DSH 远控伴侣启动");

            let listener = bridge::bind()
                .map_err(|error| setup_error(format!("VR 截图服务启动失败: {error}")))?;
            let port = listener
                .local_addr()
                .map_err(|error| setup_error(format!("VR 截图服务启动失败: {error}")))?
                .port();
            let bridge_info = BridgeInfo {
                port,
                token: random_token(),
            };
            let loaded = settings::load();
            let initial_opacity = loaded.window_opacity;
            let initial_boss_key = loaded.boss_key.clone();
            #[cfg(target_os = "linux")]
            let initial_language = loaded.language.clone();

            app.manage(AppState {
                runtime: Mutex::new(RuntimeState::new(loaded)),
                bridge: bridge_info,
                capture_lock: Arc::new(Mutex::new(())),
            });

            bridge::start(app.handle().clone(), listener);

            #[cfg(target_os = "linux")]
            let (initial_width, initial_height, min_width, min_height) = {
                let monitor = app.primary_monitor().ok().flatten();
                let (logical_width, logical_height) = monitor
                    .as_ref()
                    .map(|monitor| {
                        let scale = monitor.scale_factor().max(1.0);
                        (
                            monitor.size().width as f64 / scale,
                            monitor.size().height as f64 / scale,
                        )
                    })
                    .unwrap_or((960.0, 600.0));
                let width = 960.0_f64.min((logical_width - 160.0).max(480.0));
                let height = 600.0_f64.min((logical_height - 100.0).max(320.0));
                (width, height, width.min(800.0), height.min(500.0))
            };
            #[cfg(not(target_os = "linux"))]
            let (initial_width, initial_height, min_width, min_height) =
                (960.0, 600.0, 800.0, 500.0);

            let window = WindowBuilder::new(app, layout::WINDOW_LABEL)
                .title("DSH 远控伴侣")
                .inner_size(initial_width, initial_height)
                .min_inner_size(min_width, min_height)
                .decorations(false)
                .resizable(true)
                .shadow(true)
                .center()
                .visible(false)
                .background_color(tauri::window::Color(11, 15, 22, 255))
                .build()?;

            #[cfg(target_os = "linux")]
            {
                linux_webview::prepare(&window, app.handle().clone());
                linux_webview::set_language(app.handle(), &initial_language);
            }

            layout::create_chrome_webview(app.handle()).map_err(setup_error)?;

            if let Err(error) = window_behavior::apply_opacity(&window, initial_opacity) {
                logging::warn(&format!("无法应用窗口透明度: {error}"));
            }

            let handle = app.handle().clone();
            window.on_window_event(move |event| {
                if matches!(
                    event,
                    WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. }
                ) {
                    let app = handle.clone();
                    let runtime = app
                        .state::<AppState>()
                        .runtime
                        .lock()
                        .map(|runtime| runtime.clone())
                        .unwrap_or_else(|poisoned| poisoned.into_inner().clone());
                    if let Err(error) = layout::sync(&app, &runtime) {
                        logging::warn(&format!("窗口布局更新失败: {error}"));
                    }
                }
            });

            window.show()?;
            window_behavior::start_autohide(app.handle().clone());
            theme::start(app.handle().clone());

            if !initial_boss_key.is_empty() {
                let app_handle = app.handle().clone();
                std::thread::spawn(move || {
                    if let Err(error) =
                        window_behavior::apply_boss_key(&app_handle, &initial_boss_key)
                    {
                        logging::warn(&format!("老板键注册失败: {error}"));
                    }
                });
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
