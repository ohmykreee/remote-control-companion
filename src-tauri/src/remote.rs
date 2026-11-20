use serde_json::json;
use sha2::{Digest, Sha256};

use crate::settings::Settings;
use crate::state::BridgeInfo;

const DEFAULT_BROWSER_ARGS: &str = "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection";

pub fn normalize_remote_url(raw: &str) -> Result<tauri::Url, String> {
    let value = raw.trim();
    if value.is_empty() {
        return Err("请先输入远控地址。".to_string());
    }
    let with_scheme = if value.contains("://") {
        value.to_string()
    } else {
        format!("http://{value}")
    };
    let url = tauri::Url::parse(&with_scheme)
        .map_err(|_| "请输入完整的远控地址，例如 http://192.168.1.20:38765/。".to_string())?;
    if url.scheme() != "http" && url.scheme() != "https" {
        return Err("远控地址需要以 http:// 或 https:// 开头。".to_string());
    }
    if url.host_str().is_none() {
        return Err("远控地址缺少主机名，请检查后重试。".to_string());
    }
    Ok(url)
}

/// The WebView2 command line for the remote page. A separate browser profile is
/// used for every argument set because WebView2 requires that when browser
/// arguments differ.
pub fn browser_args(settings: &Settings, url: &tauri::Url) -> (String, bool) {
    let mut args = DEFAULT_BROWSER_ARGS.to_string();
    let mut secure_applied = false;
    if settings.force_secure_context && url.scheme() == "http" {
        let origin = url.origin().ascii_serialization();
        args.push_str(" --unsafely-treat-insecure-origin-as-secure=");
        args.push_str(&origin);
        secure_applied = true;
    }
    if cfg!(debug_assertions) {
        // The local chrome owns the environment debug port from
        // WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS. The isolated remote profile
        // needs its own port, otherwise WebView2 silently drops the second
        // bind and the remote page has no DevTools target.
        args = DEFAULT_BROWSER_ARGS.to_string();
        if secure_applied {
            args.push_str(" --unsafely-treat-insecure-origin-as-secure=");
            args.push_str(&url.origin().ascii_serialization());
        }
        let port = std::env::var("DSH_COMPANION_REMOTE_DEBUG_PORT")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "9223".to_string());
        args.push_str(" --remote-debugging-port=");
        args.push_str(port.trim());
    } else if let Ok(extra) = std::env::var("WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS") {
        let extra = extra.trim();
        if !extra.is_empty() {
            args.push(' ');
            args.push_str(extra);
        }
    }
    (args, secure_applied)
}

pub fn webview_profile(args: &str) -> String {
    let digest = Sha256::digest(args.as_bytes());
    let mut suffix = String::with_capacity(16);
    for byte in digest.iter().take(8) {
        use std::fmt::Write;
        let _ = write!(suffix, "{byte:02x}");
    }
    format!("remote-{suffix}")
}

pub fn injection_script(settings: &Settings, url: &tauri::Url, bridge: &BridgeInfo) -> String {
    let bridge_url = format!(
        "ws://127.0.0.1:{}/bridge?token={}",
        bridge.port, bridge.token
    );
    let config = json!({
        "origin": url.origin().ascii_serialization(),
        "microphoneLabel": settings.microphone_label,
        "vrScreenshot": settings.replace_clipboard_with_vr,
        "reduceMotion": settings.reduce_motion,
        "bridgeUrl": bridge_url,
    });

    let vr_api = if settings.replace_clipboard_with_vr {
        include_str!("../assets/remote_vr_api.js").to_string()
    } else {
        String::new()
    };

    include_str!("../assets/remote_bootstrap.js")
        .replace("__DSH_COMPANION_CONFIG__", &config.to_string())
        .replace("__DSH_COMPANION_VR_API__", &vr_api)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adds_http_scheme_when_missing() {
        let url = normalize_remote_url("192.168.1.20:38765").expect("valid");
        assert_eq!(url.scheme(), "http");
        assert_eq!(url.host_str(), Some("192.168.1.20"));
    }

    #[test]
    fn rejects_non_web_schemes() {
        assert!(normalize_remote_url("file:///C:/secret.txt").is_err());
    }

    #[test]
    fn secure_argument_only_applies_to_http() {
        let mut settings = Settings {
            force_secure_context: true,
            ..Settings::default()
        };
        let http = normalize_remote_url("http://192.168.1.20:38765/").unwrap();
        let (args, applied) = browser_args(&settings, &http);
        assert!(applied);
        assert!(
            args.contains("--unsafely-treat-insecure-origin-as-secure=http://192.168.1.20:38765")
        );

        let https = normalize_remote_url("https://example.com/").unwrap();
        settings.force_secure_context = true;
        let (args, applied) = browser_args(&settings, &https);
        assert!(!applied);
        assert!(!args.contains("unsafely-treat"));
    }

    #[test]
    fn injection_contains_api_only_when_enabled() {
        let mut settings = Settings::default();
        let url = normalize_remote_url("http://127.0.0.1:38765/").unwrap();
        let bridge = BridgeInfo {
            port: 1234,
            token: "token".into(),
        };
        let disabled = injection_script(&settings, &url, &bridge);
        assert!(!disabled.contains("captureVrScreenshot"));

        settings.replace_clipboard_with_vr = true;
        let enabled = injection_script(&settings, &url, &bridge);
        assert!(enabled.contains("captureVrScreenshot"));
        assert!(enabled.contains("ws://127.0.0.1:1234/bridge?token=token"));
    }
}
