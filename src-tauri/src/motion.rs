//! Propagates the local reduced-motion setting into the remote WebView using
//! Chromium's DevTools emulation. This makes the remote page's own
//! `prefers-reduced-motion` media query and listeners observe the setting.

use tauri::{AppHandle, Manager};

use crate::state::AppState;

/// Applies the setting to the currently connected remote page, if any.
pub fn apply_to_current_remote(app: &AppHandle, enabled: bool) {
    let label = {
        let state = app.state::<AppState>();
        let runtime = state
            .runtime
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        runtime.remote_label.clone()
    };
    if let Some(label) = label {
        if let Some(webview) = app.get_webview(&label) {
            apply_to_webview(&webview, enabled);
        }
    }
}

/// Applies the setting to a concrete remote WebView. Call this again whenever a
/// new remote WebView is created (connect or refresh).
pub fn apply_to_webview(webview: &tauri::Webview, enabled: bool) {
    #[cfg(windows)]
    {
        use webview2_com::CallDevToolsProtocolMethodCompletedHandler;
        use windows::core::HSTRING;

        let params = if enabled {
            r#"{"media":"","features":[{"name":"prefers-reduced-motion","value":"reduce"}]}"#
        } else {
            // Empty features clears our override, so the page follows the
            // operating system preference again.
            r#"{"features":[]}"#
        };

        let _ = webview.with_webview(move |platform| {
            let controller = platform.controller();
            let core = match unsafe { controller.CoreWebView2() } {
                Ok(core) => core,
                Err(error) => {
                    crate::logging::warn(&format!("无法同步减少动态效果: {error}"));
                    return;
                }
            };
            let handler = CallDevToolsProtocolMethodCompletedHandler::create(Box::new(
                |_result: windows::core::Result<()>, _json: String| Ok(()),
            ));
            let method = HSTRING::from("Emulation.setEmulatedMedia");
            let arguments = HSTRING::from(params);
            if let Err(error) =
                unsafe { core.CallDevToolsProtocolMethod(&method, &arguments, &handler) }
            {
                crate::logging::warn(&format!("无法同步减少动态效果: {error}"));
            }
        });
    }
    #[cfg(target_os = "linux")]
    {
        // WebKitGTK has no CDP media emulation. The injected page helper adds
        // a stylesheet that disables animations and smooth scrolling for the
        // lifetime of this remote page.
        let script = format!(
            "window.__DSH_SET_REDUCED_MOTION__ && window.__DSH_SET_REDUCED_MOTION__({});",
            if enabled { "true" } else { "false" }
        );
        if let Err(error) = webview.eval(&script) {
            crate::logging::warn(&format!("无法同步减少动态效果: {error}"));
        }
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    let _ = (webview, enabled);
}
