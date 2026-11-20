use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

use crate::paths;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub version: u32,
    pub remote_url: String,
    pub remember_address: bool,
    pub microphone_id: String,
    pub microphone_label: String,
    pub replace_clipboard_with_vr: bool,
    pub delay_vr_capture: bool,
    pub force_secure_context: bool,
    pub window_opacity: u8,
    pub boss_key: String,
    pub edge_autohide: bool,
    pub edge_autohide_edges: Vec<String>,
    pub reduce_motion: bool,
    pub language: String,
}

pub const MIN_WINDOW_OPACITY: u8 = 5;
pub const EDGE_SIDES: [&str; 3] = ["top", "left", "right"];

impl Default for Settings {
    fn default() -> Self {
        Self {
            version: 1,
            remote_url: String::new(),
            remember_address: true,
            microphone_id: String::new(),
            microphone_label: String::new(),
            replace_clipboard_with_vr: false,
            delay_vr_capture: false,
            force_secure_context: false,
            window_opacity: 100,
            boss_key: String::new(),
            edge_autohide: false,
            edge_autohide_edges: vec!["right".to_string()],
            reduce_motion: false,
            language: "zh".to_string(),
        }
    }
}

impl Settings {
    pub fn sanitize(&mut self) {
        self.version = 1;
        self.remote_url = self.remote_url.trim().to_string();
        self.microphone_id = self.microphone_id.trim().to_string();
        self.microphone_label = self.microphone_label.trim().to_string();
        if !self.remember_address {
            // The active session keeps its address in memory; only the
            // remembered value is cleared from disk.
            self.remote_url.clear();
        }
        if self.microphone_id.len() > 1024 {
            self.microphone_id.truncate(1024);
        }
        if self.microphone_label.len() > 512 {
            self.microphone_label.truncate(512);
        }
        if self.remote_url.len() > 4096 {
            self.remote_url.truncate(4096);
        }
        if !self.replace_clipboard_with_vr {
            self.delay_vr_capture = false;
        }
        self.window_opacity = self.window_opacity.clamp(MIN_WINDOW_OPACITY, 100);
        self.boss_key = self.boss_key.trim().to_string();
        if self.boss_key.len() > 64 {
            self.boss_key.truncate(64);
        }
        self.language = if self.language.eq_ignore_ascii_case("en") {
            "en".to_string()
        } else {
            "zh".to_string()
        };
        let mut edges = Vec::new();
        for edge in &self.edge_autohide_edges {
            let normalized = edge.trim().to_ascii_lowercase();
            if EDGE_SIDES.contains(&normalized.as_str()) && !edges.contains(&normalized) {
                edges.push(normalized);
            }
        }
        self.edge_autohide_edges = edges;
    }
}

pub fn load() -> Settings {
    let path = paths::settings_path();
    match fs::read_to_string(&path) {
        Ok(text) => match serde_json::from_str::<Settings>(&text) {
            Ok(mut settings) => {
                settings.sanitize();
                settings
            }
            Err(error) => {
                crate::logging::warn(&format!("settings.json 无法读取，使用默认设置: {error}"));
                Settings::default()
            }
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Settings::default(),
        Err(error) => {
            crate::logging::warn(&format!("settings.json 读取失败，使用默认设置: {error}"));
            Settings::default()
        }
    }
}

pub fn save(settings: &Settings) -> Result<(), String> {
    let path = paths::settings_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!("无法创建设置目录，请把程序放在可写目录后重试（{error}）。")
        })?;
    }
    write_atomic(&path, settings)
}

fn write_atomic(path: &Path, settings: &Settings) -> Result<(), String> {
    let text = serde_json::to_string_pretty(settings)
        .map_err(|error| format!("设置内容无法保存，请重试（{error}）。"))?;
    let temp = path.with_extension("json.tmp");
    fs::write(&temp, text)
        .map_err(|error| format!("设置未能写入 Data 目录，请检查写入权限后重试（{error}）。"))?;
    if fs::rename(&temp, path).is_err() {
        // Windows can refuse to replace an existing file if it is momentarily
        // open. Removing first keeps the portable contract without changing
        // where state lives.
        let _ = fs::remove_file(path);
        fs::rename(&temp, path)
            .map_err(|error| format!("设置未能保存，请检查 Data 目录权限后重试（{error}）。"))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_clamps_opacity_and_normalizes_edges() {
        let mut settings = Settings {
            window_opacity: 0,
            edge_autohide_edges: vec!["LEFT".to_string(), "left".to_string(), "center".to_string()],
            ..Settings::default()
        };
        settings.sanitize();
        assert_eq!(settings.window_opacity, MIN_WINDOW_OPACITY);
        assert_eq!(settings.edge_autohide_edges, vec!["left".to_string()]);
    }

    #[test]
    fn sanitize_allows_no_edge_directions() {
        let mut settings = Settings {
            edge_autohide_edges: Vec::new(),
            ..Settings::default()
        };
        settings.sanitize();
        assert!(settings.edge_autohide_edges.is_empty());
    }
}
