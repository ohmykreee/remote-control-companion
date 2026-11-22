use base64::Engine;
use serde::Serialize;
use std::time::Duration;
use tauri::{AppHandle, Manager};

use crate::backdrop;
use crate::layout;
use crate::logging;
use crate::motion;
use crate::remote;
use crate::settings::{self, Settings};
use crate::state::{AppState, RuntimeState};
use crate::window_behavior;

#[derive(Debug, Clone, Serialize)]
pub struct AppInfo {
    pub name: String,
    pub version: String,
    pub platform: String,
    pub data_dir: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct LayoutState {
    pub mode: String,
    pub overlay: bool,
    pub address: String,
    pub secure_context_applied: bool,
    pub force_secure_context: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Bootstrap {
    pub app: AppInfo,
    pub settings: Settings,
    pub layout: LayoutState,
    pub test_open_url: Option<String>,
}

fn app_info() -> AppInfo {
    AppInfo {
        name: "DSH 远控伴侣".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        platform: if cfg!(windows) {
            "Windows 10/11 · x64".to_string()
        } else if cfg!(target_os = "linux") {
            "Linux · x64".to_string()
        } else {
            std::env::consts::OS.to_string()
        },
        data_dir: crate::paths::data_dir().to_string_lossy().into_owned(),
    }
}

fn layout_state(runtime: &RuntimeState) -> LayoutState {
    let mode = if runtime.remote_url.is_some() {
        "remote"
    } else {
        "home"
    };
    let address = runtime
        .remote_url
        .clone()
        .or_else(|| {
            if runtime.settings.remember_address {
                Some(runtime.settings.remote_url.clone())
            } else {
                None
            }
        })
        .unwrap_or_default();
    LayoutState {
        mode: mode.to_string(),
        overlay: runtime.overlay,
        address,
        secure_context_applied: runtime.secure_context_applied,
        force_secure_context: runtime.settings.force_secure_context,
    }
}

fn lock(state: &AppState) -> std::sync::MutexGuard<'_, RuntimeState> {
    state
        .runtime
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[tauri::command]
pub fn get_bootstrap(app: AppHandle) -> Bootstrap {
    let state = app.state::<AppState>();
    let runtime = lock(&state);
    Bootstrap {
        app: app_info(),
        settings: runtime.settings.clone(),
        layout: layout_state(&runtime),
        test_open_url: std::env::var("DSH_COMPANION_TEST_OPEN_URL")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
    }
}

#[tauri::command]
pub async fn save_settings(app: AppHandle, settings: Settings) -> Result<Settings, String> {
    tauri::async_runtime::spawn_blocking(move || save_settings_blocking(app, settings))
        .await
        .map_err(|error| format!("保存设置时任务中断，请重试（{error}）。"))?
}

fn save_settings_blocking(app: AppHandle, settings: Settings) -> Result<Settings, String> {
    let mut next = settings;
    next.sanitize();
    settings::save(&next)?;
    let state = app.state::<AppState>();
    let previous = {
        let mut runtime = lock(&state);
        let previous = runtime.settings.clone();
        runtime.settings = next.clone();
        previous
    };
    #[cfg(target_os = "linux")]
    crate::linux_webview::set_language(&app, &next.language);

    if let Some(window) = app.get_window(layout::WINDOW_LABEL) {
        window_behavior::apply_opacity(&window, next.window_opacity)?;
    }
    if previous.boss_key != next.boss_key {
        window_behavior::change_boss_key(&app, &previous.boss_key, &next.boss_key)?;
    }
    if previous.reduce_motion != next.reduce_motion {
        motion::apply_to_current_remote(&app, next.reduce_motion);
    }
    if !next.edge_autohide {
        if let Some(window) = app.get_window(layout::WINDOW_LABEL) {
            window_behavior::restore_hidden_window(&window);
        }
    }
    Ok(next)
}

#[tauri::command]
pub fn set_window_opacity(app: AppHandle, opacity: u8) -> Result<(), String> {
    let window = app
        .get_window(layout::WINDOW_LABEL)
        .ok_or_else(|| "主窗口没有准备好。".to_string())?;
    window_behavior::apply_opacity(&window, opacity)
}

#[tauri::command]
pub async fn go_home(app: AppHandle) -> Result<LayoutState, String> {
    tauri::async_runtime::spawn_blocking(move || go_home_blocking(app))
        .await
        .map_err(|error| format!("返回连接页时任务中断，请重试（{error}）。"))?
}

fn go_home_blocking(app: AppHandle) -> Result<LayoutState, String> {
    let state = app.state::<AppState>();
    let mut runtime = lock(&state);
    if let Some(label) = runtime.remote_label.take() {
        if let Some(webview) = app.get_webview(&label) {
            let _ = webview.hide();
            let _ = webview.close();
        }
    }
    runtime.remote_url = None;
    runtime.secure_context_applied = false;
    #[cfg(target_os = "linux")]
    {
        runtime.secure_proxy = None;
    }
    runtime.overlay = false;
    layout::sync(&app, &runtime)?;
    crate::language::reset_theme(&app);
    Ok(layout_state(&runtime))
}

#[tauri::command]
pub async fn open_remote(
    app: AppHandle,
    address: String,
    remember: bool,
) -> Result<LayoutState, String> {
    tauri::async_runtime::spawn_blocking(move || open_remote_blocking(app, address, remember))
        .await
        .map_err(|error| format!("打开远控页面时任务中断，请重试（{error}）。"))?
}

fn open_remote_blocking(
    app: AppHandle,
    address: String,
    remember: bool,
) -> Result<LayoutState, String> {
    let url = remote::normalize_remote_url(&address)?;
    let state = app.state::<AppState>();
    let bridge = state.bridge.clone();
    let mut runtime = lock(&state);

    let mut next_settings = runtime.settings.clone();
    next_settings.remember_address = remember;
    next_settings.remote_url = if remember {
        url.to_string()
    } else {
        String::new()
    };
    if next_settings.auto_secure_context
        && remote::is_private_http_url(&url)
        && !next_settings.force_secure_context
    {
        next_settings.force_secure_context = true;
        logging::info("检测到内网 HTTP 地址，已自动开启强制安全上下文");
    }
    next_settings.sanitize();
    settings::save(&next_settings)?;

    layout::create_or_replace_remote(&app, &mut runtime, &url, &next_settings, &bridge)?;
    runtime.settings = next_settings;
    logging::info(&format!(
        "已打开远控页面 {}（安全上下文豁免: {}）",
        logging::redact_url(url.as_str()),
        runtime.secure_context_applied
    ));
    Ok(layout_state(&runtime))
}

#[tauri::command]
pub async fn refresh_remote(app: AppHandle) -> Result<LayoutState, String> {
    tauri::async_runtime::spawn_blocking(move || refresh_remote_blocking(app))
        .await
        .map_err(|error| format!("重新载入远控页面时任务中断，请重试（{error}）。"))?
}

fn refresh_remote_blocking(app: AppHandle) -> Result<LayoutState, String> {
    let bridge = {
        let state = app.state::<AppState>();
        state.bridge.clone()
    };
    let state = app.state::<AppState>();
    let mut runtime = lock(&state);
    let address = runtime
        .remote_url
        .clone()
        .ok_or_else(|| "当前没有可刷新的远控页面。".to_string())?;
    let url = remote::normalize_remote_url(&address)?;
    let settings = runtime.settings.clone();
    layout::create_or_replace_remote(&app, &mut runtime, &url, &settings, &bridge)?;
    logging::info(&format!(
        "已重新载入远控页面 {}",
        logging::redact_url(url.as_str())
    ));
    Ok(layout_state(&runtime))
}

#[tauri::command]
pub async fn capture_remote_backdrop(app: AppHandle) -> Result<Option<String>, String> {
    let label = {
        let state = app.state::<AppState>();
        let runtime = lock(&state);
        if runtime.remote_url.is_none() || runtime.overlay {
            return Ok(None);
        }
        runtime.remote_label.clone()
    };
    let label = label.ok_or_else(|| "远控页面还没有准备好。".to_string())?;
    let webview = app
        .get_webview(&label)
        .ok_or_else(|| "远控页面没有准备好，请刷新后重试。".to_string())?;

    let (sender, receiver) = tokio::sync::oneshot::channel();
    webview
        .with_webview(move |platform| backdrop::capture_preview(platform, sender))
        .map_err(|error| format!("无法截取远控画面（{error}）。"))?;

    let result = tokio::time::timeout(Duration::from_secs(2), receiver)
        .await
        .map_err(|_| "截取远控画面超时。".to_string())?
        .map_err(|_| "截取远控画面被中断。".to_string())??;
    if result.is_empty() {
        return Err("远控画面为空，已使用纯色背景。".to_string());
    }
    Ok(Some(
        base64::engine::general_purpose::STANDARD.encode(result),
    ))
}

#[tauri::command]
pub async fn set_chrome_overlay(app: AppHandle, active: bool) -> Result<LayoutState, String> {
    tauri::async_runtime::spawn_blocking(move || set_chrome_overlay_blocking(app, active))
        .await
        .map_err(|error| format!("窗口布局更新中断，请重试（{error}）。"))?
}

fn set_chrome_overlay_blocking(app: AppHandle, active: bool) -> Result<LayoutState, String> {
    let state = app.state::<AppState>();
    let mut runtime = lock(&state);
    if runtime.remote_url.is_some() {
        runtime.overlay = active;
        layout::sync(&app, &runtime)?;
    } else {
        runtime.overlay = false;
    }
    Ok(layout_state(&runtime))
}

#[tauri::command]
pub fn quit_app(app: AppHandle) {
    app.exit(0);
}
