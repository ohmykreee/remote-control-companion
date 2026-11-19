use std::path::{Path, PathBuf};
use std::sync::OnceLock;

static ROOT: OnceLock<PathBuf> = OnceLock::new();

fn find_dev_root() -> Option<PathBuf> {
    let cwd = std::env::current_dir().ok()?;
    let mut dir: Option<&Path> = Some(cwd.as_path());
    while let Some(current) = dir {
        if current.join("package.json").is_file() && current.join("src-tauri").is_dir() {
            return Some(current.to_path_buf());
        }
        dir = current.parent();
    }
    None
}

/// The portable install root. In development this is the project root; in a
/// packaged build it is the folder containing the executable.
pub fn app_root() -> PathBuf {
    ROOT.get_or_init(|| {
        if cfg!(debug_assertions) {
            if let Some(root) = find_dev_root() {
                return root;
            }
        }
        if let Ok(exe) = std::env::current_exe() {
            if let Some(parent) = exe.parent() {
                return parent.to_path_buf();
            }
        }
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    })
    .clone()
}

pub fn data_dir() -> PathBuf {
    if let Ok(value) = std::env::var("DSH_COMPANION_DATA_DIR") {
        if !value.trim().is_empty() {
            return PathBuf::from(value);
        }
    }
    app_root().join("Data")
}

pub fn cache_dir() -> PathBuf {
    data_dir().join("Cache")
}

pub fn logs_dir() -> PathBuf {
    data_dir().join("Logs")
}

pub fn screenshots_dir() -> PathBuf {
    cache_dir().join("VRScreenshots")
}

pub fn chrome_webview_dir() -> PathBuf {
    cache_dir().join("Webview").join("chrome")
}

pub fn remote_webview_dir(profile: &str) -> PathBuf {
    cache_dir().join("Webview").join(profile)
}

pub fn ensure_data_dirs() -> std::io::Result<()> {
    std::fs::create_dir_all(logs_dir())?;
    std::fs::create_dir_all(screenshots_dir())?;
    std::fs::create_dir_all(chrome_webview_dir())?;
    Ok(())
}

pub fn settings_path() -> PathBuf {
    data_dir().join("settings.json")
}
