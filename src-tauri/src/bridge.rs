use std::sync::{Arc, Mutex};

use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use tauri::{AppHandle, Manager};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::tungstenite::handshake::server::{ErrorResponse, Request, Response};
use tokio_tungstenite::tungstenite::http::StatusCode;
use tokio_tungstenite::tungstenite::Message;

use crate::capture;
use crate::state::AppState;

/// Binds the loopback-only bridge before the app state is managed. The
/// WebSocket token is random per launch and only injected into the trusted
/// remote page.
pub fn bind() -> Result<std::net::TcpListener, String> {
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0))
        .map_err(|error| format!("无法启动 VR 截图服务，请重启本软件（{error}）。"))?;
    listener
        .set_nonblocking(true)
        .map_err(|error| format!("无法启动 VR 截图服务，请重启本软件（{error}）。"))?;
    Ok(listener)
}

pub fn start(app: AppHandle, listener: std::net::TcpListener) {
    tauri::async_runtime::spawn(async move {
        let listener = match TcpListener::from_std(listener) {
            Ok(listener) => listener,
            Err(error) => {
                crate::logging::error(&format!("VR 截图服务监听失败: {error}"));
                return;
            }
        };
        loop {
            match listener.accept().await {
                Ok((stream, _addr)) => {
                    let app = app.clone();
                    tauri::async_runtime::spawn(async move {
                        if let Err(error) = handle_connection(app, stream).await {
                            crate::logging::warn(&format!("VR 截图连接结束: {error}"));
                        }
                    });
                }
                Err(error) => {
                    crate::logging::warn(&format!("VR 截图服务等待连接失败: {error}"));
                    tokio::time::sleep(std::time::Duration::from_millis(250)).await;
                }
            }
        }
    });
}

async fn handle_connection(app: AppHandle, stream: TcpStream) -> Result<(), String> {
    let token = app.state::<AppState>().bridge.token.clone();
    let callback = move |request: &Request, response: Response| -> Result<Response, ErrorResponse> {
        let authorized = request
            .uri()
            .query()
            .and_then(token_from_query)
            .map(|candidate| constant_time_eq(candidate.as_bytes(), token.as_bytes()))
            .unwrap_or(false);
        if authorized {
            Ok(response)
        } else {
            let mut denied = tokio_tungstenite::tungstenite::http::Response::new(Some(
                "Unauthorized".to_string(),
            ));
            *denied.status_mut() = StatusCode::UNAUTHORIZED;
            Err(denied)
        }
    };

    let mut websocket = tokio_tungstenite::accept_hdr_async(stream, callback)
        .await
        .map_err(|error| format!("连接没有通过校验: {error}"))?;

    while let Some(message) = websocket.next().await {
        let message = message.map_err(|error| error.to_string())?;
        let text = match message {
            Message::Text(text) => text,
            Message::Close(_) => break,
            Message::Ping(payload) => {
                websocket
                    .send(Message::Pong(payload))
                    .await
                    .map_err(|error| error.to_string())?;
                continue;
            }
            _ => continue,
        };

        let request: serde_json::Value = match serde_json::from_str(&text) {
            Ok(value) => value,
            Err(_) => continue,
        };
        if request.get("type").and_then(|value| value.as_str()) != Some("capture-vr-screenshot") {
            continue;
        }
        let id = request
            .get("id")
            .and_then(|value| value.as_str())
            .unwrap_or("0")
            .to_string();

        let state = app.state::<AppState>();
        let settings = state
            .runtime
            .lock()
            .map(|runtime| runtime.settings.clone())
            .unwrap_or_default();
        if !settings.replace_clipboard_with_vr {
            send_error(&mut websocket, &id, "VR 截图已关闭，请在伴侣软件的设置中开启。").await?;
            continue;
        }

        let lock: Arc<Mutex<()>> = state.capture_lock.clone();
        let result = tokio::task::spawn_blocking(move || {
            let _guard = lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            capture::capture(&settings)
        })
        .await
        .map_err(|error| format!("VR 截图任务没有完成: {error}"))?;

        match result {
            Ok(image) => send_image(&mut websocket, &id, image).await?,
            Err(error) => send_error(&mut websocket, &id, &error).await?,
        }
    }

    Ok(())
}

fn token_from_query(query: &str) -> Option<&str> {
    for part in query.split('&') {
        let Some((key, value)) = part.split_once('=') else {
            continue;
        };
        if key == "token" {
            return Some(value);
        }
    }
    None
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let mut diff = 0u8;
    for (a, b) in left.iter().zip(right.iter()) {
        diff |= a ^ b;
    }
    diff == 0
}

async fn send_image<S>(
    websocket: &mut tokio_tungstenite::WebSocketStream<S>,
    id: &str,
    image: Vec<u8>,
) -> Result<(), String>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    let metadata = json!({
        "id": id,
        "ok": true,
        "mime": "image/png",
    })
    .to_string();
    let mut payload = Vec::with_capacity(4 + metadata.len() + image.len());
    payload.extend_from_slice(&(metadata.len() as u32).to_le_bytes());
    payload.extend_from_slice(metadata.as_bytes());
    payload.extend_from_slice(&image);
    websocket
        .send(Message::Binary(payload.into()))
        .await
        .map_err(|error| format!("截图没有发送到远控页面: {error}"))
}

async fn send_error<S>(
    websocket: &mut tokio_tungstenite::WebSocketStream<S>,
    id: &str,
    message: &str,
) -> Result<(), String>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    let payload = json!({
        "id": id,
        "ok": false,
        "error": message,
    })
    .to_string();
    websocket
        .send(Message::Text(payload.into()))
        .await
        .map_err(|error| format!("错误信息没有发送到远控页面: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_token_from_query() {
        assert_eq!(token_from_query("a=1&token=abc&b=2"), Some("abc"));
        assert_eq!(token_from_query("token="), Some(""));
        assert_eq!(token_from_query("other=1"), None);
    }

    #[test]
    fn compares_without_short_circuit_on_content() {
        assert!(constant_time_eq(b"secret", b"secret"));
        assert!(!constant_time_eq(b"secret", b"secrets"));
        assert!(!constant_time_eq(b"secret", b"secreu"));
    }
}
