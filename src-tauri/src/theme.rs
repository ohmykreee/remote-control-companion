//! Reads the remote page's declared theme (dsh-remote-control sets
//! `body[data-theme]` and the `theme-color` meta tag) and forwards it to the
//! local titlebar so the chrome matches a light or dark remote page.

use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager};

use crate::state::AppState;

const POLL_INTERVAL: Duration = Duration::from_millis(500);
const QUERY_TIMEOUT: Duration = Duration::from_secs(2);

const THEME_SCRIPT: &str = r#"(() => {
  try {
    const body = document.body;
    const html = document.documentElement;
    const explicit = body && body.dataset ? body.dataset.theme : '';
    if (explicit === 'light' || explicit === 'dark') return explicit;
    if (html.classList.contains('light')) return 'light';
    if (html.classList.contains('dark')) return 'dark';

    const meta = document.querySelector('meta[name="theme-color"]');
    const metaColor = meta && meta.content ? meta.content.trim().toLowerCase() : '';
    if (metaColor === '#eef2f8') return 'light';
    if (metaColor === '#151a23') return 'dark';

    const parse = (value) => {
      const match = String(value || '').match(/rgba?\(([^)]+)\)/i);
      if (match) {
        const parts = match[1].split(',').map((part) => parseFloat(part.trim()));
        if (parts.length >= 3 && parts.every((part) => Number.isFinite(part))) {
          return [parts[0], parts[1], parts[2], parts.length > 3 ? parts[3] : 1];
        }
        return null;
      }
      const hex = String(value || '').match(/^#([0-9a-f]{6})$/i);
      if (!hex) return null;
      const number = parseInt(hex[1], 16);
      return [(number >> 16) & 255, (number >> 8) & 255, number & 255, 1];
    };
    const luminance = (rgb) => {
      const channel = (value) => {
        const v = value / 255;
        return v <= 0.03928 ? v / 12.92 : Math.pow((v + 0.055) / 1.055, 2.4);
      };
      return 0.2126 * channel(rgb[0]) + 0.7152 * channel(rgb[1]) + 0.0722 * channel(rgb[2]);
    };

    const metaRgb = parse(metaColor);
    if (metaRgb) return luminance(metaRgb) > 0.55 ? 'light' : 'dark';

    const bodyColor = body ? getComputedStyle(body).backgroundColor : '';
    const htmlColor = html ? getComputedStyle(html).backgroundColor : '';
    const rgb = parse(bodyColor === 'rgba(0, 0, 0, 0)' ? htmlColor : bodyColor);
    if (rgb && rgb[3] > 0.1) return luminance(rgb) > 0.55 ? 'light' : 'dark';
  } catch {}
  return 'dark';
})()"#;

pub fn start(app: AppHandle) {
    #[cfg(windows)]
    std::thread::Builder::new()
        .name("remote-theme".to_string())
        .spawn(move || watch_theme(app))
        .ok();
}

#[cfg(windows)]
fn watch_theme(app: AppHandle) {
    let mut last = "dark";
    loop {
        std::thread::sleep(POLL_INTERVAL);
        let label = {
            let state = app.state::<AppState>();
            let runtime = state
                .runtime
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            runtime.remote_label.clone()
        };

        let Some(label) = label else {
            if last != "dark" {
                let _ = app.emit_to(crate::layout::CHROME_LABEL, "remote-theme", "dark");
                last = "dark";
            }
            continue;
        };

        let Some(webview) = app.get_webview(&label) else {
            continue;
        };
        let Some(theme) = query_theme(&webview) else {
            // A page that is still loading or a webview that just disappeared
            // should not flash the titlebar to dark.
            continue;
        };
        if theme != last {
            let _ = app.emit_to(crate::layout::CHROME_LABEL, "remote-theme", theme);
            last = theme;
        }
    }
}

#[cfg(windows)]
fn query_theme(webview: &tauri::Webview) -> Option<&'static str> {
    use std::sync::mpsc;

    use webview2_com::ExecuteScriptCompletedHandler;
    use windows::core::HSTRING;

    let (sender, receiver) = mpsc::channel();
    let _ = webview
        .with_webview(move |platform| {
            let controller = platform.controller();
            let core = match unsafe { controller.CoreWebView2() } {
                Ok(core) => core,
                Err(error) => {
                    crate::logging::warn(&format!("无法读取远控页面主题: {error}"));
                    return;
                }
            };
            let handler = ExecuteScriptCompletedHandler::create(Box::new(move |result, json| {
                let theme = if result.is_ok() {
                    parse_theme(&json)
                } else {
                    None
                };
                let _ = sender.send(theme);
                Ok(())
            }));
            let script = HSTRING::from(THEME_SCRIPT);
            if let Err(error) = unsafe { core.ExecuteScript(&script, &handler) } {
                crate::logging::warn(&format!("无法查询远控页面主题: {error}"));
            }
        })
        .ok()?;

    receiver.recv_timeout(QUERY_TIMEOUT).ok().flatten()
}

fn parse_theme(json: &str) -> Option<&'static str> {
    let value: String = serde_json::from_str(json).ok()?;
    match value.as_str() {
        "light" => Some("light"),
        "dark" => Some("dark"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::parse_theme;

    #[test]
    fn parses_the_theme_result() {
        assert_eq!(parse_theme("\"light\""), Some("light"));
        assert_eq!(parse_theme("\"dark\""), Some("dark"));
        assert_eq!(parse_theme("null"), None);
        assert_eq!(parse_theme("not json"), None);
    }
}
