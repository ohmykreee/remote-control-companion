use std::fs::{self, OpenOptions};
use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::paths;

fn stamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

fn write(level: &str, message: &str) {
    let dir = paths::logs_dir();
    let _ = fs::create_dir_all(&dir);
    let path = dir.join("session.log");
    if let Ok(metadata) = fs::metadata(&path) {
        if metadata.len() > 2 * 1024 * 1024 {
            let _ = fs::remove_file(&path);
        }
    }
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "[{}] [{level}] {message}", stamp());
    }
    if cfg!(debug_assertions) {
        println!("[{level}] {message}");
    }
}

pub fn info(message: &str) {
    write("info", message);
}

pub fn warn(message: &str) {
    write("warn", message);
}

pub fn error(message: &str) {
    write("error", message);
}

/// URLs may carry the remote-control pairing token in their query string.
/// Logs keep the origin and path but replace the query entirely.
pub fn redact_url(value: &str) -> String {
    match tauri::Url::parse(value) {
        Ok(mut url) => {
            if url.query().is_some() {
                url.set_query(Some("redacted"));
            }
            url.to_string()
        }
        Err(_) => "<invalid-url>".to_string(),
    }
}
