use std::fs;

use tauri::webview::{PermissionResponse, WebviewBuilder};
use tauri::{AppHandle, Manager, WebviewUrl};

use crate::logging;
use crate::paths;
use crate::remote;
use crate::state::BridgeInfo;

pub const WINDOW_LABEL: &str = "main";
pub const CHROME_LABEL: &str = "chrome";
pub const TITLEBAR_HEIGHT: f64 = 44.0;

pub fn client_logical_size(window: &tauri::Window) -> Result<(f64, f64), String> {
    let size = window
        .inner_size()
        .map_err(|error| format!("无法读取窗口尺寸（{error}）。"))?;
    let scale = window.scale_factor().unwrap_or(1.0);
    Ok((
        size.width as f64 / scale,
        size.height as f64 / scale,
    ))
}
fn get_window(app: &AppHandle) -> Result<tauri::Window, String> {
    app.get_window(WINDOW_LABEL)
        .ok_or_else(|| "主窗口没有准备好，请重新打开本软件。".to_string())
}

pub fn sync(app: &AppHandle, runtime: &crate::state::RuntimeState) -> Result<(), String> {
    let window = get_window(app)?;
    let (width, height) = client_logical_size(&window)?;
    let chrome = app
        .get_webview(CHROME_LABEL)
        .ok_or_else(|| "窗口内容没有准备好，请重新打开本软件。".to_string())?;
    let remote_webview = runtime
        .remote_label
        .as_deref()
        .and_then(|label| app.get_webview(label));

    let show_full = runtime.remote_url.is_none() || runtime.overlay;

    if show_full {
        if let Some(remote) = &remote_webview {
            let _ = remote.hide();
        }
        chrome
            .set_position(tauri::LogicalPosition::new(0.0, 0.0))
            .map_err(|error| format!("无法调整窗口布局（{error}）。"))?;
        chrome
            .set_size(tauri::LogicalSize::new(
                width.max(1.0),
                height.max(1.0),
            ))
            .map_err(|error| format!("无法调整窗口布局（{error}）。"))?;
    } else {
        // Shrink the chrome first; the remote webview is shown only after the
        // titlebar-only strip is in place so the two never overlap.
        chrome
            .set_position(tauri::LogicalPosition::new(0.0, 0.0))
            .map_err(|error| format!("无法调整窗口布局（{error}）。"))?;
        chrome
            .set_size(tauri::LogicalSize::new(
                width.max(1.0),
                TITLEBAR_HEIGHT.min(height.max(1.0)),
            ))
            .map_err(|error| format!("无法调整窗口布局（{error}）。"))?;

        if let Some(remote) = &remote_webview {
            remote
                .set_position(tauri::LogicalPosition::new(0.0, TITLEBAR_HEIGHT))
                .map_err(|error| format!("无法调整远控页面（{error}）。"))?;
            remote
                .set_size(tauri::LogicalSize::new(
                    width.max(1.0),
                    (height - TITLEBAR_HEIGHT).max(1.0),
                ))
                .map_err(|error| format!("无法调整远控页面（{error}）。"))?;
            let _ = remote.show();
        }
    }
    Ok(())
}

pub fn create_chrome_webview(app: &AppHandle) -> Result<(), String> {
    let window = get_window(app)?;
    let (width, height) = client_logical_size(&window)?;
    let data_dir = paths::chrome_webview_dir();
    fs::create_dir_all(&data_dir)
        .map_err(|error| format!("无法创建缓存目录，请检查 Data 目录权限（{error}）。"))?;

    let builder = WebviewBuilder::new(CHROME_LABEL, WebviewUrl::App("index.html".into()))
        .data_directory(data_dir)
        .enable_clipboard_access()
        .on_permission_request(|_, _| PermissionResponse::Allow)
        .disable_drag_drop_handler();

    #[cfg(all(windows, debug_assertions))]
    let builder = builder.additional_browser_args(
        "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection --remote-debugging-port=9222",
    );

    window
        .add_child(
            builder,
            tauri::LogicalPosition::new(0.0, 0.0),
            tauri::LogicalSize::new(width.max(1.0), height.max(1.0)),
        )
        .map_err(|error| format!("无法创建主窗口内容（{error}）。"))?;
    Ok(())
}

pub fn create_or_replace_remote(
    app: &AppHandle,
    runtime: &mut crate::state::RuntimeState,
    url: &tauri::Url,
    settings: &crate::settings::Settings,
    bridge: &BridgeInfo,
) -> Result<(), String> {
    let window = get_window(app)?;
    let (width, height) = client_logical_size(&window)?;
    let next_generation = runtime.generation + 1;
    let label = format!("remote-{next_generation}");

    let (args, secure_applied) = remote::browser_args(settings, url);
    logging::info(&format!(
        "远端 WebView 参数准备: profile={}, secure={}, args={}",
        remote::webview_profile(&args),
        secure_applied,
        args
    ));
    let profile = remote::webview_profile(&args);
    let data_dir = paths::remote_webview_dir(&profile);
    fs::create_dir_all(&data_dir)
        .map_err(|error| format!("无法创建远控页面缓存，请检查 Data 目录权限（{error}）。"))?;

    let script = remote::injection_script(settings, url, bridge);
    let mut builder = WebviewBuilder::new(&label, WebviewUrl::External(url.clone()))
        .data_directory(data_dir)
        .enable_clipboard_access()
        .on_permission_request(|_, _| PermissionResponse::Allow)
        .initialization_script(script)
        .disable_drag_drop_handler();

    #[cfg(windows)]
    {
        builder = builder.additional_browser_args(&args);
    }

    let new_webview = window
        .add_child(
            builder,
            tauri::LogicalPosition::new(0.0, TITLEBAR_HEIGHT),
            tauri::LogicalSize::new(
                width.max(1.0),
                (height - TITLEBAR_HEIGHT).max(1.0),
            ),
        )
        .map_err(|error| format!("无法打开远控页面，请确认地址可访问（{error}）。"))?;

    if let Some(old_label) = runtime.remote_label.take() {
        if let Some(old) = app.get_webview(&old_label) {
            let _ = old.hide();
            let _ = old.close();
        }
    }

    runtime.remote_label = Some(label);
    runtime.remote_url = Some(url.to_string());
    runtime.secure_context_applied = secure_applied;
    runtime.generation = next_generation;
    runtime.overlay = false;

    // Ensure the new webview is not painted underneath a full-size chrome
    // viewport after a reload triggered from settings.
    drop(new_webview);
    sync(app, runtime)?;
    Ok(())
}
