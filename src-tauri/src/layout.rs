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
    Ok((size.width as f64 / scale, size.height as f64 / scale))
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

    #[cfg(target_os = "linux")]
    {
        let _ = (width, height);
        crate::linux_webview::configure(&window, &chrome, remote_webview.as_ref(), show_full);
    }

    #[cfg(not(target_os = "linux"))]
    {
        if show_full {
            if let Some(remote) = &remote_webview {
                let _ = remote.hide();
            }
            chrome
                .set_position(tauri::LogicalPosition::new(0.0, 0.0))
                .map_err(|error| format!("无法调整窗口布局（{error}）。"))?;
            chrome
                .set_size(tauri::LogicalSize::new(width.max(1.0), height.max(1.0)))
                .map_err(|error| format!("无法调整窗口布局（{error}）。"))?;
        } else {
            // Shrink the chrome first; the remote webview is shown only after
            // the titlebar-only strip is in place so the two never overlap.
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

    let chrome = window
        .add_child(
            builder,
            tauri::LogicalPosition::new(0.0, 0.0),
            tauri::LogicalSize::new(width.max(1.0), height.max(1.0)),
        )
        .map_err(|error| format!("无法创建主窗口内容（{error}）。"))?;
    #[cfg(target_os = "linux")]
    crate::linux_webview::configure(&window, &chrome, None, true);
    #[cfg(not(target_os = "linux"))]
    let _ = &chrome;
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

    let (args, secure_from_args) = remote::browser_args(settings, url);
    #[cfg(target_os = "linux")]
    let (page_url, secure_proxy) = remote::linux_page_url(url, settings)?;
    #[cfg(not(target_os = "linux"))]
    let page_url = url.clone();
    #[cfg(target_os = "linux")]
    let secure_applied = secure_proxy.is_some();
    #[cfg(target_os = "linux")]
    let _ = secure_from_args;
    #[cfg(not(target_os = "linux"))]
    let secure_applied = secure_from_args;
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

    let script = remote::state_script(settings, &page_url);
    let user_agent = remote::companion_user_agent(app);
    if let Some(user_agent) = &user_agent {
        logging::info(&format!("远端 User-Agent: {user_agent}"));
    } else {
        logging::warn("无法读取 WebView 默认 User-Agent，未能附加 RC-Companion 标记");
    }
    let builder = WebviewBuilder::new(&label, WebviewUrl::External(page_url.clone()))
        .data_directory(data_dir)
        .enable_clipboard_access()
        .on_permission_request(|_, _| PermissionResponse::Allow)
        .initialization_script(script)
        .disable_drag_drop_handler();
    // The screenshot API is the only conditional injection; every state
    // report lives in the single bootstrap script above.
    let builder = if settings.replace_clipboard_with_vr {
        builder.initialization_script(remote::vr_api_script(bridge))
    } else {
        builder
    };
    let builder = match user_agent {
        Some(user_agent) => builder.user_agent(&user_agent),
        None => builder,
    };

    #[cfg(windows)]
    let builder = builder.additional_browser_args(&args);

    let new_webview = window
        .add_child(
            builder,
            tauri::LogicalPosition::new(0.0, TITLEBAR_HEIGHT),
            tauri::LogicalSize::new(width.max(1.0), (height - TITLEBAR_HEIGHT).max(1.0)),
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
    #[cfg(target_os = "linux")]
    {
        runtime.secure_proxy = secure_proxy.map(std::sync::Arc::new);
        crate::linux_webview::install_media_permissions(&new_webview);
    }
    runtime.generation = next_generation;
    runtime.overlay = false;

    // Ensure the new webview is not painted underneath a full-size chrome
    // viewport after a reload triggered from settings.
    crate::motion::apply_to_webview(&new_webview, settings.reduce_motion);
    crate::language::install(&new_webview, app.clone());
    drop(new_webview);
    sync(app, runtime)?;
    Ok(())
}
