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
}

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
                crate::logging::warn(&format!(
                    "settings.json 无法读取，使用默认设置: {error}"
                ));
                Settings::default()
            }
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Settings::default(),
        Err(error) => {
            crate::logging::warn(&format!(
                "settings.json 读取失败，使用默认设置: {error}"
            ));
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
    fs::write(&temp, text).map_err(|error| {
        format!("设置未能写入 Data 目录，请检查写入权限后重试（{error}）。")
    })?;
    if fs::rename(&temp, path).is_err() {
        // Windows can refuse to replace an existing file if it is momentarily
        // open. Removing first keeps the portable contract without changing
        // where state lives.
        let _ = fs::remove_file(path);
        fs::rename(&temp, path).map_err(|error| {
            format!("设置未能保存，请检查 Data 目录权限后重试（{error}）。")
        })?;
    }
    Ok(())
}
