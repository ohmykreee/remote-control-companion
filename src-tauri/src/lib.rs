mod backdrop;
mod bridge;
mod capture;
mod commands;
mod layout;
mod logging;
mod paths;
mod remote;
mod settings;
mod state;

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
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .invoke_handler(tauri::generate_handler![
            commands::get_bootstrap,
            commands::save_settings,
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

            app.manage(AppState {
                runtime: Mutex::new(RuntimeState::new(loaded)),
                bridge: bridge_info,
                capture_lock: Arc::new(Mutex::new(())),
            });

            bridge::start(app.handle().clone(), listener);

            let window = WindowBuilder::new(app, layout::WINDOW_LABEL)
                .title("DSH 远控伴侣")
                .inner_size(960.0, 600.0)
                .min_inner_size(800.0, 500.0)
                .decorations(false)
                .resizable(true)
                .shadow(true)
                .center()
                .visible(false)
                .background_color(tauri::window::Color(11, 15, 22, 255))
                .build()?;

            layout::create_chrome_webview(app.handle())
                .map_err(setup_error)?;

            let handle = app.handle().clone();
            window.on_window_event(move |event| {
                if matches!(event, WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. }) {
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
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
