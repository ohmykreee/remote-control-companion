//! Minimal OSC 1.0 encoder and UDP delivery for VRChat's chatbox endpoint.
//!
//! VRChat listens for OSC on UDP 9000 by default and shows `/chatbox/input`
//! text in the in-game chatbox. The message is an address string, an OSC type
//! tag string, and the string argument, all padded to four bytes.

use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};

/// VRChat's chatbox endpoint for text.
pub const CHATBOX_INPUT: &str = "/chatbox/input";

/// The chatbox drops text longer than this many characters.
pub const CHATBOX_LIMIT: usize = 144;

/// Encode `/chatbox/input` with the text and "send immediately".
///
/// The official OSC documentation defines two arguments: the text and whether
/// VRChat should send it right away. Sending it immediately keeps the chatbox
/// showing the panel without opening the keyboard.
pub fn chatbox_message(text: &str) -> Vec<u8> {
    let mut payload = Vec::with_capacity(text.len() + 32);
    write_osc_string(&mut payload, CHATBOX_INPUT);
    write_osc_string(&mut payload, ",sT");
    write_osc_string(&mut payload, text);
    payload
}

/// OSC strings are NUL terminated and padded with NULs to a multiple of four
/// bytes, including at least one terminator when the length is already aligned.
fn write_osc_string(payload: &mut Vec<u8>, value: &str) {
    payload.extend_from_slice(value.as_bytes());
    let padding = 4 - (value.len() % 4);
    payload.extend(std::iter::repeat_n(0u8, padding));
}

/// A UDP socket that caches the resolved target so a host name is not resolved
/// again for every refresh.
#[derive(Default)]
pub struct UdpSender {
    socket: Option<(bool, UdpSocket)>,
    target: Option<(String, u16, SocketAddr)>,
}

impl UdpSender {
    pub fn new() -> Self {
        Self::default()
    }

    /// Forget the cached target and socket. The next send resolves again and
    /// rebuilds the socket for the target address family.
    pub fn reset(&mut self) {
        self.socket = None;
        self.target = None;
    }

    pub fn send(&mut self, host: &str, port: u16, message: &[u8]) -> Result<(), String> {
        let address = self.resolve(host, port)?;
        let socket = self.socket_for(address)?;
        socket
            .send_to(message, address)
            .map(|_| ())
            .map_err(|error| format!("无法发送到 {host}:{port}（{error}）。"))
    }

    fn resolve(&mut self, host: &str, port: u16) -> Result<SocketAddr, String> {
        if let Some((cached_host, cached_port, address)) = &self.target {
            if cached_host == host && *cached_port == port {
                return Ok(*address);
            }
        }
        let mut addresses = (host, port)
            .to_socket_addrs()
            .map_err(|error| format!("无法识别发送地址 {host}:{port}（{error}）。"))?;
        let address = addresses
            .next()
            .ok_or_else(|| format!("发送地址 {host}:{port} 没有可用的网络地址。"))?;
        self.target = Some((host.to_string(), port, address));
        Ok(address)
    }

    fn socket_for(&mut self, address: SocketAddr) -> Result<&UdpSocket, String> {
        let want_ipv6 = address.is_ipv6();
        let rebuild = match &self.socket {
            Some((current_ipv6, _)) => *current_ipv6 != want_ipv6,
            None => true,
        };
        if rebuild {
            let socket = if want_ipv6 {
                UdpSocket::bind(("::", 0))
            } else {
                UdpSocket::bind(("0.0.0.0", 0))
            }
            .map_err(|error| format!("无法准备发送端口（{error}）。"))?;
            self.socket = Some((want_ipv6, socket));
        }
        Ok(&self.socket.as_ref().expect("socket was just created").1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chatbox_message_has_address_typetag_and_text() {
        let payload = chatbox_message("hi");
        assert_eq!(&payload[..16], b"/chatbox/input\0\0");
        assert_eq!(&payload[16..20], b",sT\0");
        assert_eq!(&payload[20..], b"hi\0\0");
    }

    #[test]
    fn osc_strings_are_padded_to_four_bytes() {
        let mut payload = Vec::new();
        write_osc_string(&mut payload, "ab");
        assert_eq!(payload, b"ab\0\0");
        payload.clear();
        write_osc_string(&mut payload, "abcd");
        assert_eq!(payload, b"abcd\0\0\0\0");
    }

    #[test]
    fn longer_text_is_kept_intact() {
        let text = "x".repeat(140);
        let payload = chatbox_message(&text);
        assert!(payload.ends_with(b"x\0\0\0\0"));
    }
}
