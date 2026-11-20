//! Receives a small language hint from the remote page through WebView2's
//! web-message channel. This avoids granting the remote page access to the
//! Tauri command surface just to report its language.

use tauri::{AppHandle, Emitter};

pub fn install(webview: &tauri::Webview, app: AppHandle) {
    #[cfg(windows)]
    {
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
            let handler = WebMessageReceivedEventHandler::create(Box::new(
                move |_sender, args| {
                    if let Some(args) = args {
                        let mut value = PWSTR::null();
                        if unsafe { args.TryGetWebMessageAsString(&mut value) }.is_ok() {
                            if let Ok(text) = unsafe { value.to_string() } {
                                handle_message(&app, &text);
                            }
                        }
                    }
                    Ok(())
                },
            ));
            let mut token = 0i64;
            if let Err(error) =
                unsafe { core.add_WebMessageReceived(&handler, &mut token) }
            {
                crate::logging::warn(&format!("无法监听远控页面语言: {error}"));
            }
        });
    }
    #[cfg(not(windows))]
    let _ = (webview, app);
}

fn handle_message(app: &AppHandle, text: &str) {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(text) else {
        return;
    };
    if value.get("type").and_then(|value| value.as_str()) != Some("companion-language") {
        return;
    }
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
