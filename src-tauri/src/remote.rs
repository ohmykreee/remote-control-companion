use serde_json::json;
use sha2::{Digest, Sha256};
use tauri::Manager;

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

/// Append the companion marker to the WebView's actual default User-Agent.
/// The base string is read from the already-created local chrome WebView, so
/// engine versions and platform tokens stay exactly as the runtime reports
/// them instead of being hard-coded here.
pub fn companion_user_agent(app: &tauri::AppHandle) -> Option<String> {
    let base = read_chrome_user_agent(app)?;
    let base = base.trim();
    if base.is_empty() {
        return None;
    }
    Some(append_companion(base))
}

fn append_companion(base: &str) -> String {
    if base.contains("RC-Companion") {
        return base.to_string();
    }
    format!("{base} RC-Companion/{}", env!("CARGO_PKG_VERSION"))
}

#[cfg(target_os = "linux")]
fn read_chrome_user_agent(app: &tauri::AppHandle) -> Option<String> {
    use std::sync::mpsc;
    use std::time::Duration;

    use webkit2gtk::{SettingsExt, WebViewExt};

    let webview = app.get_webview(crate::layout::CHROME_LABEL)?;
    let (sender, receiver) = mpsc::channel();
    webview
        .with_webview(move |platform| {
            let value = WebViewExt::settings(&platform.inner())
                .and_then(|settings| settings.user_agent())
                .map(|value| value.to_string());
            let _ = sender.send(value);
        })
        .ok()?;
    receiver.recv_timeout(Duration::from_secs(2)).ok().flatten()
}

#[cfg(windows)]
fn read_chrome_user_agent(app: &tauri::AppHandle) -> Option<String> {
    use std::ffi::c_void;
    use std::sync::mpsc;
    use std::time::Duration;

    use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Settings2;
    use windows::core::{Interface as _, PWSTR};
    use windows::Win32::System::Com::CoTaskMemFree;

    let webview = app.get_webview(crate::layout::CHROME_LABEL)?;
    let (sender, receiver) = mpsc::channel();
    webview
        .with_webview(move |platform| {
            let value = (|| -> Option<String> {
                let core = unsafe { platform.controller().CoreWebView2().ok()? };
                let settings = unsafe { core.Settings().ok()? };
                let settings: ICoreWebView2Settings2 = settings.cast().ok()?;
                let mut raw = PWSTR::null();
                unsafe { settings.UserAgent(&mut raw).ok()? };
                let value = unsafe { raw.to_string().ok() };
                unsafe { CoTaskMemFree(Some(raw.0 as *const c_void)) };
                value
            })();
            let _ = sender.send(value);
        })
        .ok()?;
    receiver.recv_timeout(Duration::from_secs(2)).ok().flatten()
}

#[cfg(not(any(target_os = "linux", windows)))]
fn read_chrome_user_agent(_app: &tauri::AppHandle) -> Option<String> {
    None
}

/// True for a plain-HTTP URL whose host is a private/loopback IP address.
/// Hostnames are intentionally not resolved here: DNS could change between
/// this check and the actual request.
pub fn is_private_http_url(url: &tauri::Url) -> bool {
    if url.scheme() != "http" {
        return false;
    }
    match url.host() {
        Some(url::Host::Ipv4(address)) => {
            address.is_private()
                || address.is_loopback()
                || address.is_link_local()
                || is_shared_ipv4(address)
        }
        Some(url::Host::Ipv6(address)) => {
            address.is_loopback() || address.is_unique_local() || address.is_unicast_link_local()
        }
        _ => false,
    }
}

fn is_shared_ipv4(address: std::net::Ipv4Addr) -> bool {
    let octets = address.octets();
    octets[0] == 100 && (64..128).contains(&octets[1])
}

#[cfg(target_os = "linux")]
pub fn linux_page_url(
    url: &tauri::Url,
    settings: &Settings,
) -> Result<(tauri::Url, Option<crate::loopback_proxy::LoopbackProxy>), String> {
    if !settings.force_secure_context || url.scheme() != "http" {
        return Ok((url.clone(), None));
    }
    let host = url
        .host_str()
        .ok_or_else(|| "远控地址缺少主机名。".to_string())?;
    let port = url
        .port_or_known_default()
        .ok_or_else(|| "远控地址缺少端口。".to_string())?;
    let proxy = crate::loopback_proxy::LoopbackProxy::start(host, port)?;
    let mut page = url.clone();
    page.set_host(Some("127.0.0.1"))
        .map_err(|_| "无法创建本地安全上下文地址。".to_string())?;
    page.set_port(Some(proxy.port()))
        .map_err(|_| "无法创建本地安全上下文端口。".to_string())?;
    Ok((page, Some(proxy)))
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

pub fn state_script(settings: &Settings, url: &tauri::Url) -> String {
    let config = json!({
        "origin": url.origin().ascii_serialization(),
        "microphoneLabel": settings.microphone_label,
        "reduceMotion": settings.reduce_motion,
    });

    include_str!("../assets/remote_bootstrap.js")
        .replace("__DSH_COMPANION_CONFIG__", &config.to_string())
}

/// The VR screenshot API is the only part injected conditionally: it is a
/// separate script so enabling it never changes the single state-reporting
/// injection.
pub fn vr_api_script(bridge: &BridgeInfo) -> String {
    let bridge_url = format!(
        "ws://127.0.0.1:{}/bridge?token={}",
        bridge.port, bridge.token
    );
    let config = json!({
        "vrScreenshot": true,
        "bridgeUrl": bridge_url,
    });
    format!(
        "(function () {{
  var config = {};
{}
}})();
",
        config,
        include_str!("../assets/remote_vr_api.js")
    )
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
    fn companion_marker_keeps_original_ua_and_is_not_duplicated() {
        let base = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Safari/605.1.15";
        let ua = append_companion(base);
        assert!(ua.starts_with(base));
        assert!(ua.contains("RC-Companion/"));
        assert_eq!(append_companion(&ua), ua);
    }

    #[test]
    fn private_http_address_detection() {
        for address in [
            "http://10.1.2.3:38765/",
            "http://172.16.3.119:38765/",
            "http://192.168.1.20/",
            "http://127.0.0.1:38765/",
            "http://100.64.0.1/",
            "http://[::1]:38765/",
            "http://[fd00::1]/",
        ] {
            assert!(is_private_http_url(&normalize_remote_url(address).unwrap()));
        }
        for address in [
            "https://172.16.3.119:38765/",
            "http://8.8.8.8/",
            "http://example.com/",
        ] {
            assert!(!is_private_http_url(
                &normalize_remote_url(address).unwrap()
            ));
        }
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
    fn state_script_never_contains_the_screenshot_api() {
        let settings = Settings::default();
        let url = normalize_remote_url("http://127.0.0.1:38765/").unwrap();
        let script = state_script(&settings, &url);
        assert!(!script.contains("captureVrScreenshot"));
        assert!(!script.contains("bridgeUrl"));
    }

    #[test]
    fn vr_api_script_is_injected_separately() {
        let bridge = BridgeInfo {
            port: 1234,
            token: "token".into(),
        };
        let script = vr_api_script(&bridge);
        assert!(script.contains("captureVrScreenshot"));
        assert!(script.contains("ws://127.0.0.1:1234/bridge?token=token"));
    }
}
