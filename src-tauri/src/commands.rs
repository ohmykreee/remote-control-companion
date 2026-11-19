use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::layout;
use crate::logging;
use crate::remote;
use crate::settings::{self, Settings};
use crate::state::{AppState, RuntimeState};

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
}

#[derive(Debug, Clone, Serialize)]
pub struct Bootstrap {
    pub app: AppInfo,
    pub settings: Settings,
    pub layout: LayoutState,
}

fn app_info() -> AppInfo {
    AppInfo {
        name: "DSH 远控伴侣".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        platform: if cfg!(windows) {
            "Windows 10/11 · x64".to_string()
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
    }
}

#[tauri::command]
pub fn save_settings(settings: Settings, app: AppHandle) -> Result<Settings, String> {
    let mut next = settings;
    next.sanitize();
    settings::save(&next)?;
    let state = app.state::<AppState>();
    let mut runtime = lock(&state);
    runtime.settings = next.clone();
    Ok(next)
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
