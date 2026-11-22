use std::io::Cursor;
use std::time::Duration;

#[cfg(any(windows, target_os = "linux"))]
use std::ffi::CString;
#[cfg(any(windows, target_os = "linux"))]
use std::fs;
#[cfg(any(windows, target_os = "linux"))]
use std::path::Path;
#[cfg(any(windows, target_os = "linux", test))]
use std::path::PathBuf;
#[cfg(any(windows, target_os = "linux"))]
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use crate::settings::Settings;

#[cfg(any(windows, target_os = "linux"))]
const VR_APPLICATION_OVERLAY: i32 = 2;
#[cfg(any(windows, target_os = "linux"))]
const VR_INIT_ERROR_NONE: i32 = 0;
#[cfg(any(windows, target_os = "linux"))]
const SCREENSHOT_INTERFACE: &str = "FnTable:IVRScreenshots_001";
#[cfg(any(windows, target_os = "linux"))]
const SCREENSHOT_TIMEOUT: Duration = Duration::from_secs(10);
const COUNTDOWN_SOUND: &[u8] = include_bytes!("../assets/capture-countdown.wav");

/// The SteamVR preview is a near-square eye texture with a large unrendered
/// area below the real view. Keep only the top portion, estimated from the
/// red-line reference at 57.2% of the image height.
const VR_PREVIEW_TOP_RATIO: f64 = 0.572;

/// Blocks for the screenshot operation. Called from a blocking worker on the
/// bridge, never directly from Tauri's async runtime.
pub fn capture(settings: &Settings) -> Result<Vec<u8>, String> {
    if settings.delay_vr_capture {
        three_second_countdown();
    }

    #[cfg(windows)]
    {
        platform::capture_openvr()
    }
    #[cfg(target_os = "linux")]
    {
        platform::capture_linux()
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        Err("VR 截图目前只支持 Windows 和 Linux。".to_string())
    }
}

fn three_second_countdown() {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Media::Audio::{PlaySoundW, SND_ASYNC, SND_MEMORY, SND_NODEFAULT};

        // The embedded clip contains three countdown beeps at one-second
        // intervals and the shutter click at the 3.0s mark.
        unsafe {
            let _ = PlaySoundW(
                COUNTDOWN_SOUND.as_ptr() as *const u16,
                std::ptr::null_mut(),
                SND_MEMORY | SND_ASYNC | SND_NODEFAULT,
            );
        }
        std::thread::sleep(Duration::from_secs(3));
    }

    #[cfg(target_os = "linux")]
    {
        if let Some(path) = write_countdown_file() {
            for player in ["pw-play", "paplay"] {
                if std::process::Command::new(player)
                    .arg(&path)
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn()
                    .is_ok()
                {
                    break;
                }
            }
        }
        std::thread::sleep(Duration::from_secs(3));
    }

    #[cfg(not(any(windows, target_os = "linux")))]
    std::thread::sleep(Duration::from_secs(3));
}

#[cfg(target_os = "linux")]
fn write_countdown_file() -> Option<std::path::PathBuf> {
    let path = crate::paths::cache_dir().join("capture-countdown.wav");
    if std::fs::write(&path, COUNTDOWN_SOUND).is_ok() {
        Some(path)
    } else {
        None
    }
}

fn crop_preview(png_bytes: &[u8]) -> Result<Vec<u8>, String> {
    let image = image::load_from_memory_with_format(png_bytes, image::ImageFormat::Png)
        .map_err(|error| format!("无法读取 VR 截图（{error}）。"))?;
    let (width, height) = (image.width(), image.height());
    if width == 0 || height == 0 || (height as f64) <= (width as f64) * 0.75 {
        return Ok(png_bytes.to_vec());
    }

    let crop_height = (((height as f64) * VR_PREVIEW_TOP_RATIO).round() as u32).clamp(1, height);
    let cropped = image.crop_imm(0, 0, width, crop_height);
    let mut encoded = Cursor::new(Vec::new());
    cropped
        .write_to(&mut encoded, image::ImageFormat::Png)
        .map_err(|error| format!("无法裁剪 VR 截图（{error}）。"))?;
    Ok(encoded.into_inner())
}

#[cfg(any(windows, target_os = "linux"))]
mod platform {
    use super::*;
    use libloading::{Library, Symbol};
    use std::os::raw::{c_char, c_void};

    type FnIsRuntimeInstalled = unsafe extern "C" fn() -> bool;
    type FnIsHmdPresent = unsafe extern "C" fn() -> bool;
    type FnInit = unsafe extern "C" fn(*mut i32, i32) -> *mut c_void;
    type FnShutdown = unsafe extern "C" fn();
    type FnGetGenericInterface = unsafe extern "C" fn(*const c_char, *mut i32) -> *mut c_void;
    type FnTakeStereo = unsafe extern "C" fn(*mut u32, *const c_char, *const c_char) -> i32;

    type FnGetScreenshotFilename =
        unsafe extern "C" fn(u32, i32, *mut c_char, u32, *mut i32) -> u32;

    #[repr(C)]
    struct ScreenshotFnTable {
        _request_screenshot: usize,
        _hook_screenshot: usize,
        _get_screenshot_property_type: usize,
        get_screenshot_property_filename: Option<FnGetScreenshotFilename>,
        _update_screenshot_progress: usize,
        take_stereo_screenshot: Option<FnTakeStereo>,
        _submit_screenshot: usize,
    }

    const _: () = assert!(std::mem::size_of::<ScreenshotFnTable>() == 7 * 8);

    #[cfg(windows)]
    fn library_candidates() -> Vec<PathBuf> {
        let mut candidates = Vec::new();
        if let Ok(exe) = std::env::current_exe() {
            if let Some(parent) = exe.parent() {
                candidates.push(parent.join("openvr_api.dll"));
            }
        }

        if let Ok(local) = std::env::var("LOCALAPPDATA") {
            let vrpath = PathBuf::from(local)
                .join("openvr")
                .join("openvrpaths.vrpath");
            if let Ok(text) = fs::read_to_string(vrpath) {
                if let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) {
                    if let Some(runtimes) = value.get("runtime").and_then(|value| value.as_array())
                    {
                        for runtime in runtimes.iter().filter_map(|value| value.as_str()) {
                            candidates.push(
                                PathBuf::from(runtime)
                                    .join("bin")
                                    .join("win64")
                                    .join("openvr_api.dll"),
                            );
                        }
                    }
                }
            }
        }

        for env_name in ["ProgramFiles(x86)", "ProgramFiles"] {
            if let Ok(program_files) = std::env::var(env_name) {
                candidates.push(
                    PathBuf::from(program_files)
                        .join("Steam")
                        .join("steamapps")
                        .join("common")
                        .join("SteamVR")
                        .join("bin")
                        .join("win64")
                        .join("openvr_api.dll"),
                );
            }
        }

        candidates
            .into_iter()
            .filter(|candidate| candidate.is_file())
            .collect()
    }

    #[cfg(target_os = "linux")]
    fn library_candidates() -> Vec<PathBuf> {
        let mut candidates = Vec::new();
        if let Ok(path) = std::env::var("DSH_COMPANION_OPENVR_PATH") {
            if !path.trim().is_empty() {
                candidates.push(PathBuf::from(path));
            }
        }
        if let Ok(exe) = std::env::current_exe() {
            if let Some(parent) = exe.parent() {
                candidates.push(parent.join("libopenvr_api.so"));
                candidates.push(parent.join("openvr_api.so"));
            }
        }

        if let Ok(home) = std::env::var("HOME") {
            let home = PathBuf::from(home);
            for config in [
                ".config/openvr/openvrpaths.vrpath",
                ".openvr/openvrpaths.vrpath",
            ] {
                if let Ok(text) = fs::read_to_string(home.join(config)) {
                    if let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) {
                        if let Some(runtimes) =
                            value.get("runtime").and_then(|value| value.as_array())
                        {
                            for runtime in runtimes.iter().filter_map(|value| value.as_str()) {
                                for relative in ["lib/linux64", "bin/linux64", "lib/linux32"] {
                                    candidates.push(
                                        PathBuf::from(runtime)
                                            .join(relative)
                                            .join("libopenvr_api.so"),
                                    );
                                }
                            }
                        }
                    }
                }
            }

            for root in [
                ".steam/steam",
                ".steam/root",
                ".local/share/Steam",
                ".var/app/com.valvesoftware.Steam/.local/share/Steam",
            ] {
                for relative in ["lib/linux64", "bin/linux64"] {
                    candidates.push(
                        home.join(root)
                            .join("steamapps")
                            .join("common")
                            .join("SteamVR")
                            .join(relative)
                            .join("libopenvr_api.so"),
                    );
                }
            }
        }

        for path in [
            "/usr/lib/openvr/libopenvr_api.so",
            "/usr/lib/x86_64-linux-gnu/libopenvr_api.so",
            "/opt/steam/steamapps/common/SteamVR/lib/linux64/libopenvr_api.so",
        ] {
            candidates.push(PathBuf::from(path));
        }

        candidates
            .into_iter()
            .filter(|candidate| candidate.is_file())
            .collect()
    }

    fn timestamp() -> u128 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis())
            .unwrap_or_default()
    }

    fn find_capture_file(dir: &Path, prefix: &str) -> Option<PathBuf> {
        fs::read_dir(dir)
            .ok()?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .find(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .map(|name| name.starts_with(prefix))
                    .unwrap_or(false)
                    && path.is_file()
                    && path.metadata().map(|meta| meta.len() > 0).unwrap_or(false)
            })
    }

    fn wait_for_preview(dir: &Path) -> Option<PathBuf> {
        let deadline = Instant::now() + SCREENSHOT_TIMEOUT;
        while Instant::now() < deadline {
            if let Some(path) = find_capture_file(dir, "preview") {
                return Some(path);
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        None
    }

    pub fn capture_openvr() -> Result<Vec<u8>, String> {
        let dll = library_candidates().into_iter().next().ok_or_else(|| {
            if cfg!(windows) {
                "没有找到 SteamVR。请先启动 SteamVR，或把 openvr_api.dll 放在程序目录旁边。"
                    .to_string()
            } else {
                "没有找到 SteamVR。请先启动 SteamVR，或把 libopenvr_api.so 放在程序目录旁边。"
                    .to_string()
            }
        })?;

        let batch_dir = crate::paths::screenshots_dir().join(format!("shot-{}", timestamp()));
        fs::create_dir_all(&batch_dir)
            .map_err(|error| format!("无法创建截图临时目录，请检查 Data 目录权限（{error}）。"))?;
        let preview_base = batch_dir.join("preview.png");
        let vr_base = batch_dir.join("vr.png");

        let result = (|| -> Result<Vec<u8>, String> {
            unsafe {
                let library = Library::new(&dll).map_err(|error| {
                    format!("无法加载 SteamVR 组件，请重新启动 SteamVR 后重试（{error}）。")
                })?;

                let is_runtime_installed: Symbol<FnIsRuntimeInstalled> = library
                    .get(b"VR_IsRuntimeInstalled\0")
                    .map_err(|_| "SteamVR 组件不完整，请重新安装 SteamVR。".to_string())?;
                if !is_runtime_installed() {
                    return Err(
                        "SteamVR 还没有运行。请先启动 SteamVR，唤醒头显后重试。".to_string()
                    );
                }

                let is_hmd_present: Symbol<FnIsHmdPresent> = library
                    .get(b"VR_IsHmdPresent\0")
                    .map_err(|_| "SteamVR 组件不完整，请重新安装 SteamVR。".to_string())?;
                if !is_hmd_present() {
                    return Err(
                        "没有检测到头显。请唤醒头显并确认 SteamVR 已就绪后重试。".to_string()
                    );
                }

                let init: Symbol<FnInit> = library
                    .get(b"VR_InitInternal\0")
                    .map_err(|_| "SteamVR 组件不完整，请重新安装 SteamVR。".to_string())?;
                let shutdown: Symbol<FnShutdown> = library
                    .get(b"VR_ShutdownInternal\0")
                    .map_err(|_| "SteamVR 组件不完整，请重新安装 SteamVR。".to_string())?;
                let get_interface: Symbol<FnGetGenericInterface> = library
                    .get(b"VR_GetGenericInterface\0")
                    .map_err(|_| "SteamVR 组件不完整，请重新安装 SteamVR。".to_string())?;

                let mut init_error = VR_INIT_ERROR_NONE;
                let _system = init(&mut init_error, VR_APPLICATION_OVERLAY);
                if init_error != VR_INIT_ERROR_NONE {
                    return Err(format!(
                        "无法连接 SteamVR（错误码 {init_error}）。请先启动 SteamVR、唤醒头显后重试。"
                    ));
                }

                let outcome = (|| -> Result<Vec<u8>, String> {
                    let version = CString::new(SCREENSHOT_INTERFACE)
                        .map_err(|_| "截图接口初始化失败。".to_string())?;
                    let mut interface_error = VR_INIT_ERROR_NONE;
                    let interface = get_interface(version.as_ptr(), &mut interface_error);
                    if interface.is_null() || interface_error != VR_INIT_ERROR_NONE {
                        return Err(format!(
                            "当前 SteamVR 不支持截图接口（错误码 {interface_error}）。请更新 SteamVR 后重试。"
                        ));
                    }
                    let table = &*(interface as *const ScreenshotFnTable);
                    let take = table.take_stereo_screenshot.ok_or_else(|| {
                        "当前 SteamVR 不支持截图接口，请更新 SteamVR 后重试。".to_string()
                    })?;

                    let preview = path_as_c_string(&preview_base)?;
                    let vr = path_as_c_string(&vr_base)?;
                    let mut handle: u32 = 0;
                    let error = take(&mut handle, preview.as_ptr(), vr.as_ptr());
                    crate::logging::info(&format!(
                        "OpenVR 截图请求: handle={handle}, error={error}, dir={}",
                        batch_dir.display()
                    ));
                    if error != 0 {
                        return Err(format!(
                            "VR 截图失败（错误码 {error}）。请确认 SteamVR 正在运行、头显已唤醒，并且有 VR 应用正在显示。"
                        ));
                    }

                    if let Some(get_filename) = table.get_screenshot_property_filename {
                        let mut property_error = 0;
                        let mut buffer = vec![0i8; 1024];
                        let length = get_filename(
                            handle,
                            0,
                            buffer.as_mut_ptr(),
                            buffer.len() as u32,
                            &mut property_error,
                        );
                        let reported = if length > 0 {
                            let c_string = CString::from_vec_unchecked(
                                buffer
                                    .iter()
                                    .take_while(|byte| **byte != 0)
                                    .map(|byte| *byte as u8)
                                    .collect::<Vec<u8>>(),
                            );
                            c_string.to_string_lossy().into_owned()
                        } else {
                            String::new()
                        };
                        crate::logging::info(&format!(
                            "OpenVR 截图属性: preview={reported:?}, property_error={property_error}"
                        ));
                    }

                    let preview_file = wait_for_preview(&batch_dir).ok_or_else(|| {
                        let names = fs::read_dir(&batch_dir)
                            .map(|entries| {
                                entries
                                    .filter_map(Result::ok)
                                    .map(|entry| entry.file_name().to_string_lossy().into_owned())
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            })
                            .unwrap_or_default();
                        crate::logging::warn(&format!("VR 截图超时，目录内容: [{names}]"));
                        "等待 VR 截图超时。请先在头显中打开任意 VR 应用，再试一次。".to_string()
                    })?;
                    let full = fs::read(&preview_file)
                        .map_err(|error| format!("无法读取截图结果，请重试（{error}）。"))?;
                    match crop_preview(&full) {
                        Ok(cropped) => {
                            crate::logging::info(&format!(
                                "VR 截图已裁剪: {} -> {} bytes",
                                full.len(),
                                cropped.len()
                            ));
                            Ok(cropped)
                        }
                        Err(error) => {
                            crate::logging::warn(&format!(
                                "VR 截图裁剪失败，返回原始截图: {error}"
                            ));
                            Ok(full)
                        }
                    }
                })();

                shutdown();
                outcome
            }
        })();

        match &result {
            Ok(bytes) => crate::logging::info(&format!(
                "VR 截图成功: {} bytes, dll={}",
                bytes.len(),
                dll.display()
            )),
            Err(error) => crate::logging::warn(&format!("VR 截图失败: {error}")),
        }
        let _ = fs::remove_dir_all(&batch_dir);
        result
    }

    fn path_as_c_string(path: &Path) -> Result<CString, String> {
        CString::new(path.to_string_lossy().as_bytes())
            .map_err(|_| "截图路径包含无效字符，请把程序移动到更简单的目录后重试。".to_string())
    }

    #[cfg(target_os = "linux")]
    pub fn capture_linux() -> Result<Vec<u8>, String> {
        let mock = std::env::var("DSH_COMPANION_VR_MOCK")
            .map(|value| value == "1")
            .unwrap_or(false);
        if !mock {
            return capture_openvr();
        }

        let full = mock_preview()?;
        match crop_preview(&full) {
            Ok(cropped) => Ok(cropped),
            Err(error) => {
                crate::logging::warn(&format!("VR 测试截图裁剪失败，返回原始截图: {error}"));
                Ok(full)
            }
        }
    }

    #[cfg(target_os = "linux")]
    pub fn mock_preview() -> Result<Vec<u8>, String> {
        let width = 1024;
        let height = 1024;
        let image = image::RgbaImage::from_fn(width, height, |x, y| {
            let red = (36 + (x * 80 / width)) as u8;
            let green = (54 + (y * 70 / height)) as u8;
            let blue = (96 + ((x + y) * 60 / (width + height))) as u8;
            let line = (y as f64 / height as f64 - 0.572).abs() < 0.002;
            if line {
                image::Rgba([210, 72, 82, 255])
            } else {
                image::Rgba([red, green, blue, 255])
            }
        });
        let mut encoded = Cursor::new(Vec::new());
        image
            .write_to(&mut encoded, image::ImageFormat::Png)
            .map_err(|error| format!("无法生成 VR 测试截图（{error}）。"))?;
        Ok(encoded.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageFormat, Rgba, RgbaImage};

    #[test]
    fn crop_preview_keeps_the_top_portion_of_a_near_square_image() {
        let source = RgbaImage::from_pixel(100, 100, Rgba([12, 34, 56, 255]));
        let mut png = Cursor::new(Vec::new());
        source.write_to(&mut png, ImageFormat::Png).unwrap();

        let cropped = crop_preview(&png.into_inner()).expect("crop");
        let decoded = image::load_from_memory_with_format(&cropped, ImageFormat::Png).unwrap();
        assert_eq!(decoded.width(), 100);
        assert_eq!(decoded.height(), 57);
    }

    #[test]
    fn crop_preview_leaves_wide_images_untouched() {
        let source = RgbaImage::from_pixel(160, 90, Rgba([1, 2, 3, 255]));
        let mut png = Cursor::new(Vec::new());
        source.write_to(&mut png, ImageFormat::Png).unwrap();
        let original = png.into_inner();
        let cropped = crop_preview(&original).expect("crop");
        assert_eq!(cropped, original);
    }

    #[test]
    #[ignore = "requires a running SteamVR session and an awake headset"]
    fn capture_openvr_smoke() {
        let bytes = capture(&Settings::default()).expect("VR capture");
        let target = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("vr-capture-test.png");
        std::fs::write(&target, &bytes).expect("write capture");
        println!("captured {} bytes to {}", bytes.len(), target.display());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_mock_preview_is_cropped_like_steamvr() {
        let full = platform::mock_preview().expect("mock preview");
        let cropped = crop_preview(&full).expect("crop");
        let decoded = image::load_from_memory_with_format(&cropped, ImageFormat::Png).unwrap();
        assert_eq!(decoded.width(), 1024);
        assert_eq!(
            decoded.height(),
            (1024.0 * VR_PREVIEW_TOP_RATIO).round() as u32
        );
    }
}
