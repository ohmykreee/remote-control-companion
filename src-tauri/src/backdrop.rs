//! Screenshot the remote WebView itself at menu-open time. WebView2's own
//! CapturePreview API avoids screen-capture problems (window occlusion, DPI
//! scaling, or capturing a menu) and returns a compact JPEG for the blurred
//! backdrop.

#[cfg(windows)]
pub fn capture_preview(
    platform: tauri::webview::PlatformWebview,
    sender: tokio::sync::oneshot::Sender<Result<Vec<u8>, String>>,
) {
    use std::ffi::c_void;

    use webview2_com::CapturePreviewCompletedHandler;
    use webview2_com::Microsoft::Web::WebView2::Win32::COREWEBVIEW2_CAPTURE_PREVIEW_IMAGE_FORMAT_JPEG;
    use windows::Win32::System::Com::{IStream, STREAM_SEEK_SET};
    use windows::Win32::UI::Shell::SHCreateMemStream;

    let controller = platform.controller();
    let core = match unsafe { controller.CoreWebView2() } {
        Ok(core) => core,
        Err(error) => {
            let _ = sender.send(Err(format!("无法读取远控页面（{error}）。")));
            return;
        }
    };
    let stream = match unsafe { SHCreateMemStream(None) } {
        Some(stream) => stream,
        None => {
            let _ = sender.send(Err("无法创建背景截图缓冲区。".to_string()));
            return;
        }
    };
    let stream_for_callback: IStream = stream.clone();

    let handler = CapturePreviewCompletedHandler::create(Box::new(move |result| {
        let outcome = (|| -> Result<Vec<u8>, String> {
            result.map_err(|error| format!("无法截取远控画面（{error}）。"))?;
            unsafe {
                stream_for_callback
                    .Seek(0, STREAM_SEEK_SET, None)
                    .map_err(|error| format!("无法读取背景截图（{error}）。"))?;
            }
            let mut bytes = Vec::new();
            let mut chunk = vec![0u8; 64 * 1024];
            loop {
                let mut read = 0u32;
                let hr = unsafe {
                    stream_for_callback.Read(
                        chunk.as_mut_ptr() as *mut c_void,
                        chunk.len() as u32,
                        Some(&mut read),
                    )
                };
                if hr.is_err() {
                    return Err(format!("无法读取背景截图（{hr:?}）。"));
                }
                if read == 0 {
                    break;
                }
                bytes.extend_from_slice(&chunk[..read as usize]);
            }
            Ok(bytes)
        })();
        if let Ok(bytes) = &outcome {
            crate::logging::info(&format!("WebView2 背景截图: {} bytes", bytes.len()));
        } else if let Err(error) = &outcome {
            crate::logging::warn(&format!("WebView2 背景截图失败: {error}"));
        }
        let _ = sender.send(outcome);
        Ok(())
    }));

    if let Err(error) = unsafe {
        core.CapturePreview(
            COREWEBVIEW2_CAPTURE_PREVIEW_IMAGE_FORMAT_JPEG,
            &stream,
            &handler,
        )
    } {
        crate::logging::warn(&format!("WebView2 背景截图请求失败: {error}"));
    }
}
#[cfg(not(windows))]
pub fn capture_preview(
    _platform: tauri::webview::PlatformWebview,
    sender: tokio::sync::oneshot::Sender<Result<Vec<u8>, String>>,
) {
    let _ = sender.send(Err("当前平台不支持背景截图。".to_string()));
}
