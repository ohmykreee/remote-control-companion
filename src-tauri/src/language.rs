//! Receives small language/theme hints from the remote page through the
//! native WebView message channel. This avoids granting the remote page access
//! to the Tauri command surface just to report its language.

use tauri::{AppHandle, Emitter};

pub fn install(webview: &tauri::Webview, app: AppHandle) {
    #[cfg(windows)]
    install_windows(webview, app);

    #[cfg(target_os = "linux")]
    install_linux(webview, app);

    #[cfg(not(any(windows, target_os = "linux")))]
    let _ = (webview, app);
}

#[cfg(windows)]
fn install_windows(webview: &tauri::Webview, app: AppHandle) {
    use webview2_com::WebMessageReceivedEventHandler;
    use windows::core::PWSTR;

    let _ = webview.with_webview(move |platform| {
        let controller = platform.controller();
        let core = match unsafe { controller.CoreWebView2() } {
            Ok(core) => core,
            Err(error) => {
                crate::logging::warn(&format!("无法监听远控页面语言: {error}"));
                return;
            }
        };
        let handler = WebMessageReceivedEventHandler::create(Box::new(move |_sender, args| {
            if let Some(args) = args {
                let mut value = PWSTR::null();
                if unsafe { args.TryGetWebMessageAsString(&mut value) }.is_ok() {
                    if let Ok(text) = unsafe { value.to_string() } {
                        handle_message(&app, &text);
                    }
                }
            }
            Ok(())
        }));
        let mut token = 0i64;
        if let Err(error) = unsafe { core.add_WebMessageReceived(&handler, &mut token) } {
            crate::logging::warn(&format!("无法监听远控页面语言: {error}"));
        }
    });
}

#[cfg(target_os = "linux")]
fn install_linux(webview: &tauri::Webview, app: AppHandle) {
    use javascriptcore::ValueExt;
    use webkit2gtk::{UserContentManagerExt, WebViewExt};

    const HANDLER: &str = "dshCompanion";

    let _ = webview.with_webview(move |platform| {
        let view = platform.inner();
        let Some(manager) = view.user_content_manager() else {
            crate::logging::warn("无法获取远控页面消息通道");
            return;
        };

        if !manager.register_script_message_handler(HANDLER) {
            crate::logging::warn("无法注册远控页面消息通道");
            return;
        }

        manager.connect_script_message_received(Some(HANDLER), move |_manager, result| {
            let Some(value) = result.js_value() else {
                return;
            };
            handle_message(&app, &value.to_str());
        });
    });
}

fn handle_message(app: &AppHandle, text: &str) {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(text) else {
        return;
    };
    let kind = value
        .get("type")
        .and_then(|value| value.as_str())
        .unwrap_or_default();

    match kind {
        "companion-language" => {
            let language = value
                .get("language")
                .and_then(|value| value.as_str())
                .unwrap_or_default();
            let language = if language.eq_ignore_ascii_case("en") {
                "en"
            } else if language.eq_ignore_ascii_case("zh") {
                "zh"
            } else {
                return;
            };
            let _ = app.emit_to(crate::layout::CHROME_LABEL, "remote-language", language);
        }
        "companion-theme" => {
            let theme = value
                .get("theme")
                .and_then(|value| value.as_str())
                .unwrap_or_default();
            if theme == "light" || theme == "dark" {
                let _ = app.emit_to(crate::layout::CHROME_LABEL, "remote-theme", theme);
            }
        }
        _ => {}
    }
}
