//! Loopback TCP bridge used to give WebKitGTK a secure origin for plain-HTTP
//! remote pages.
//!
//! WebKitGTK has no equivalent of Chromium's
//! `--unsafely-treat-insecure-origin-as-secure`. `http://127.0.0.1` *is* a
//! secure context, so the remote page is loaded through a loopback listener
//! that forwards raw HTTP/WebSocket bytes to the real HTTP server. Relative
//! URLs, `/ws`, fetch and media requests then stay on the secure localhost
//! origin.

use std::io;
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

#[derive(Debug)]
pub struct LoopbackProxy {
    port: u16,
    stop: Arc<AtomicBool>,
}

impl LoopbackProxy {
    pub fn start(host: &str, port: u16) -> Result<Self, String> {
        let target: Vec<SocketAddr> = (host, port)
            .to_socket_addrs()
            .map_err(|error| format!("无法解析远控地址，请检查网络（{error}）。"))?
            .collect();
        if target.is_empty() {
            return Err("无法解析远控地址，请检查网络。".to_string());
        }

        let listener = TcpListener::bind(("127.0.0.1", 0))
            .map_err(|error| format!("无法创建本地安全上下文转发（{error}）。"))?;
        listener
            .set_nonblocking(true)
            .map_err(|error| format!("无法配置本地安全上下文转发（{error}）。"))?;
        let local_port = listener
            .local_addr()
            .map_err(|error| format!("无法读取本地安全上下文端口（{error}）。"))?
            .port();
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = stop.clone();

        thread::Builder::new()
            .name("dsh-loopback-proxy".to_string())
            .spawn(move || {
                while !thread_stop.load(Ordering::Relaxed) {
                    match listener.accept() {
                        Ok((client, _peer)) => {
                            let target = target.clone();
                            thread::spawn(move || bridge(client, &target));
                        }
                        Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                            thread::sleep(Duration::from_millis(40));
                        }
                        Err(_) => break,
                    }
                }
            })
            .map_err(|error| format!("无法启动本地安全上下文转发（{error}）。"))?;

        Ok(Self {
            port: local_port,
            stop,
        })
    }

    pub fn port(&self) -> u16 {
        self.port
    }
}

impl Drop for LoopbackProxy {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

fn bridge(client: TcpStream, target: &[SocketAddr]) {
    let Some(upstream) = target
        .iter()
        .find_map(|address| TcpStream::connect(address).ok())
    else {
        return;
    };
    let _ = client.set_nodelay(true);
    let _ = upstream.set_nodelay(true);
    let (Ok(mut client_read), Ok(mut client_write)) = (client.try_clone(), client.try_clone())
    else {
        return;
    };
    let (Ok(mut upstream_read), Ok(mut upstream_write)) =
        (upstream.try_clone(), upstream.try_clone())
    else {
        return;
    };

    let to_upstream = thread::spawn(move || {
        let _ = io::copy(&mut client_read, &mut upstream_write);
        let _ = upstream_write.shutdown(Shutdown::Write);
    });
    let _ = io::copy(&mut upstream_read, &mut client_write);
    let _ = client_write.shutdown(Shutdown::Write);
    let _ = to_upstream.join();
}
