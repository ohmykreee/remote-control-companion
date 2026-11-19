use std::ffi::CString;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::settings::Settings;

const VR_APPLICATION_OVERLAY: i32 = 2;
const VR_INIT_ERROR_NONE: i32 = 0;
const SCREENSHOT_INTERFACE: &str = "FnTable:IVRScreenshots_001";
const SCREENSHOT_TIMEOUT: Duration = Duration::from_secs(10);

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
    #[cfg(not(windows))]
    {
        Err("VR 截图目前只支持 Windows。".to_string())
    }
}

fn three_second_countdown() {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Media::Audio::{PlaySoundW, SND_ASYNC, SND_MEMORY, SND_NODEFAULT};

        let wav = platform::beep_wav();
        for _ in 0..3 {
            unsafe {
                let _ = PlaySoundW(
                    wav.as_ptr() as *const u16,
                    std::ptr::null_mut(),
                    SND_MEMORY | SND_ASYNC | SND_NODEFAULT,
                );
            }
            std::thread::sleep(Duration::from_secs(1));
        }
        // Keep the sound buffer alive for the asynchronous player until the
        // screenshot begins.
        drop(wav);
        return;
    }

    #[cfg(not(windows))]
    std::thread::sleep(Duration::from_secs(3));
}

#[cfg(windows)]
mod platform {
    use super::*;
    use libloading::{Library, Symbol};
    use std::os::raw::{c_char, c_void};

    type FnIsRuntimeInstalled = unsafe extern "C" fn() -> bool;
    type FnIsHmdPresent = unsafe extern "C" fn() -> bool;
    type FnInit = unsafe extern "C" fn(*mut i32, i32) -> *mut c_void;
    type FnShutdown = unsafe extern "C" fn();
    type FnGetGenericInterface =
        unsafe extern "C" fn(*const c_char, *mut i32) -> *mut c_void;
    type FnTakeStereo = unsafe extern "C" fn(
        *mut u32,
        *const c_char,
        *const c_char,
    ) -> i32;

    type FnGetScreenshotFilename = unsafe extern "C" fn(
        u32,
        i32,
        *mut c_char,
        u32,
        *mut i32,
    ) -> u32;

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

    pub fn beep_wav() -> Vec<u8> {
        const SAMPLE_RATE: u32 = 44_100;
        const DURATION_MS: u32 = 120;
        const FREQUENCY: f64 = 1046.5;
        let sample_count = (SAMPLE_RATE as f64 * DURATION_MS as f64 / 1000.0) as u32;
        let data_size = sample_count * 2;
        let mut wav = Vec::with_capacity((44 + data_size) as usize);

        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36 + data_size).to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
        wav.extend_from_slice(&(SAMPLE_RATE * 2).to_le_bytes());
        wav.extend_from_slice(&2u16.to_le_bytes());
        wav.extend_from_slice(&16u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&data_size.to_le_bytes());

        for index in 0..sample_count {
            let t = index as f64 / SAMPLE_RATE as f64;
            let envelope = {
                let progress = index as f64 / sample_count as f64;
                let attack = (progress / 0.06).min(1.0);
                let release = ((1.0 - progress) / 0.28).min(1.0);
                attack.min(release)
            };
            let sample = (t * FREQUENCY * std::f64::consts::TAU).sin()
                * 0.28
                * envelope;
            wav.extend_from_slice(&((sample * i16::MAX as f64) as i16).to_le_bytes());
        }
        wav
    }

    fn library_candidates() -> Vec<PathBuf> {
        let mut candidates = Vec::new();
        if let Ok(exe) = std::env::current_exe() {
            if let Some(parent) = exe.parent() {
                candidates.push(parent.join("openvr_api.dll"));
            }
        }

        if let Ok(local) = std::env::var("LOCALAPPDATA") {
            let vrpath = PathBuf::from(local).join("openvr").join("openvrpaths.vrpath");
            if let Ok(text) = fs::read_to_string(vrpath) {
                if let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) {
                    if let Some(runtimes) = value.get("runtime").and_then(|value| value.as_array()) {
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
            "没有找到 SteamVR。请先启动 SteamVR，或把 openvr_api.dll 放在程序目录旁边。"
                .to_string()
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
                    return Err("SteamVR 还没有运行。请先启动 SteamVR，唤醒头显后重试。".to_string());
                }

                let is_hmd_present: Symbol<FnIsHmdPresent> = library
                    .get(b"VR_IsHmdPresent\0")
                    .map_err(|_| "SteamVR 组件不完整，请重新安装 SteamVR。".to_string())?;
                if !is_hmd_present() {
                    return Err("没有检测到头显。请唤醒头显并确认 SteamVR 已就绪后重试。".to_string());
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
                        crate::logging::warn(&format!(
                            "VR 截图超时，目录内容: [{names}]"
                        ));
                        "等待 VR 截图超时。请先在头显中打开任意 VR 应用，再试一次。".to_string()
                    })?;
                    fs::read(&preview_file).map_err(|error| {
                        format!("无法读取截图结果，请重试（{error}）。")
                    })
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
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

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
}
