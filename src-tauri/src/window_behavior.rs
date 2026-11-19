//! Windows-level window behaviour that Tauri does not expose directly:
//! layered opacity, the global boss key, and QQ-style edge auto-hide.

use std::time::{Duration, Instant};

use tauri::{AppHandle, Manager};

use crate::settings::MIN_WINDOW_OPACITY;
use crate::state::AppState;

const WINDOW_LABEL: &str = crate::layout::WINDOW_LABEL;
const EDGE_PEEK: i32 = 6;
const REVEAL_DISTANCE: i32 = 160;
const DOCK_DISTANCE: i32 = 10;
const HIDE_DELAY: Duration = Duration::from_millis(650);
const POLL_INTERVAL: Duration = Duration::from_millis(100);
const SLIDE_STEPS: u32 = 14;
const SLIDE_STEP_DELAY: Duration = Duration::from_millis(12);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Edge {
    Top,
    Left,
    Right,
}

impl Edge {
    fn as_str(self) -> &'static str {
        match self {
            Self::Top => "top",
            Self::Left => "left",
            Self::Right => "right",
        }
    }

    fn is_enabled(self, edges: &[String]) -> bool {
        edges.iter().any(|edge| edge == self.as_str())
    }

    fn hide_position(self, rect: &ScreenRect, work: &ScreenRect) -> (i32, i32) {
        match self {
            Self::Left => (work.left - rect.width() + EDGE_PEEK, rect.top),
            Self::Right => (work.right - EDGE_PEEK, rect.top),
            Self::Top => (rect.left, work.top - rect.height() + EDGE_PEEK),
        }
    }

    /// Snap the restored window flush to the screen edge. If the window was
    /// dragged partly past the edge, this is the position it comes back to.
    fn snapped_to_work(self, rect: &ScreenRect, work: &ScreenRect) -> ScreenRect {
        let mut normal = *rect;
        match self {
            Self::Left => normal.left = work.left,
            Self::Right => normal.left = work.right - normal.width(),
            Self::Top => normal.top = work.top,
        }
        normal
    }

    fn reveals(self, cursor: Point, normal: &ScreenRect, work: &ScreenRect) -> bool {
        let in_vertical_span =
            cursor.y >= normal.top - 12 && cursor.y <= normal.bottom + 12;
        let in_horizontal_span =
            cursor.x >= normal.left - 12 && cursor.x <= normal.right + 12;
        match self {
            Self::Left => {
                in_vertical_span && cursor.x <= work.left.saturating_add(REVEAL_DISTANCE)
            }
            Self::Right => {
                in_vertical_span && cursor.x >= work.right.saturating_sub(REVEAL_DISTANCE)
            }
            Self::Top => {
                in_horizontal_span && cursor.y <= work.top.saturating_add(REVEAL_DISTANCE)
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct ScreenRect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

impl ScreenRect {
    fn width(self) -> i32 {
        (self.right - self.left).max(1)
    }

    fn height(self) -> i32 {
        (self.bottom - self.top).max(1)
    }

    fn contains(self, point: Point) -> bool {
        point.x >= self.left
            && point.x < self.right
            && point.y >= self.top
            && point.y < self.bottom
    }
}

#[derive(Debug, Clone, Copy)]
struct Point {
    x: i32,
    y: i32,
}

#[derive(Debug, Clone)]
struct HiddenSnapshot {
    edge: Edge,
    normal: ScreenRect,
    work: ScreenRect,
}

#[derive(Default)]
struct AutoHideState {
    hidden: Option<HiddenSnapshot>,
    outside_since: Option<Instant>,
}

#[cfg(windows)]
static AUTO_HIDE: std::sync::OnceLock<std::sync::Arc<std::sync::Mutex<AutoHideState>>> =
    std::sync::OnceLock::new();

/// Applies the stored opacity to the native window. Values below 100 use a
/// layered window so the whole window, including both child webviews, fades.
#[cfg(windows)]
pub fn apply_opacity(window: &tauri::Window, opacity: u8) -> Result<(), String> {
    use windows::Win32::Foundation::COLORREF;
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindowLongW, SetLayeredWindowAttributes, SetWindowLongW, GWL_EXSTYLE, LWA_ALPHA,
        WS_EX_LAYERED,
    };

    let hwnd = window
        .hwnd()
        .map_err(|error| format!("无法读取窗口句柄（{error}）。"))?;
    let opacity = opacity.clamp(MIN_WINDOW_OPACITY, 100);
    let alpha = ((opacity as u32 * 255 + 50) / 100) as u8;
    unsafe {
        let style = GetWindowLongW(hwnd, GWL_EXSTYLE);
        SetWindowLongW(hwnd, GWL_EXSTYLE, style | WS_EX_LAYERED.0 as i32);
        SetLayeredWindowAttributes(hwnd, COLORREF(0), alpha, LWA_ALPHA)
            .map_err(|error| format!("无法调整窗口透明度（{error}）。"))?;
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn apply_opacity(_window: &tauri::Window, _opacity: u8) -> Result<(), String> {
    Ok(())
}

/// Registers the boss key with the global-shortcut plugin. The plugin dispatches
/// registration to the Tauri main thread, so callers must not already be on it.
pub fn apply_boss_key(app: &AppHandle, accelerator: &str) -> Result<(), String> {
    use tauri_plugin_global_shortcut::GlobalShortcutExt;

    let shortcut = accelerator.trim();
    let manager = app.global_shortcut();
    manager
        .unregister_all()
        .map_err(|error| format!("无法更新老板键（{error}）。"))?;
    if shortcut.is_empty() {
        return Ok(());
    }
    manager
        .register(shortcut)
        .map_err(|error| format!("老板键无法注册，可能已被其他程序占用（{error}）。"))
}

/// Replaces the boss key without losing the previous registration when the new
/// accelerator is rejected (for example because another app already owns it).
pub fn change_boss_key(
    app: &AppHandle,
    previous: &str,
    accelerator: &str,
) -> Result<(), String> {
    use tauri_plugin_global_shortcut::GlobalShortcutExt;

    if previous.trim() == accelerator.trim() {
        return Ok(());
    }
    let manager = app.global_shortcut();
    let next = accelerator.trim();
    if next.is_empty() {
        manager
            .unregister_all()
            .map_err(|error| format!("无法清除老板键（{error}）。"))?;
        return Ok(());
    }
    manager
        .register(next)
        .map_err(|error| format!("老板键无法注册，可能已被其他程序占用（{error}）。"))?;
    if !previous.trim().is_empty() {
        let _ = manager.unregister(previous.trim());
    }
    Ok(())
}

/// Called by the global-shortcut handler. The plugin fires both press and
/// release, so the handler only forwards Pressed.
pub fn toggle_boss_key(app: &AppHandle) {
    let Some(window) = app.get_window(WINDOW_LABEL) else {
        return;
    };
    let minimized = window.is_minimized().unwrap_or(false);
    let visible = window.is_visible().unwrap_or(false);
    if visible && !minimized {
        let _ = window.hide();
        return;
    }

    restore_hidden_window(&window);
    let _ = window.show();
    let _ = window.unminimize();
    let _ = window.set_focus();
}

/// Starts the edge watcher once. It only touches native Win32 window state, so
/// it is safe to run away from Tauri's event-loop thread.
pub fn start_autohide(app: AppHandle) {
    #[cfg(windows)]
    {
        let state = std::sync::Arc::new(std::sync::Mutex::new(AutoHideState::default()));
        if AUTO_HIDE.set(state.clone()).is_err() {
            return;
        }
        let _ = std::thread::Builder::new()
            .name("edge-auto-hide".to_string())
            .spawn(move || auto_hide_loop(app, state));
    }
}

/// Restores a window that the edge watcher moved mostly off-screen. Used when
/// the boss key shows the window, a second instance is launched, or auto-hide
/// is disabled.
pub fn restore_hidden_window(window: &tauri::Window) {
    #[cfg(windows)]
    {
        let Some(state) = AUTO_HIDE.get() else {
            return;
        };
        let snapshot = {
            let mut state = state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            state.hidden.take()
        };
        if let Some(snapshot) = snapshot {
            if let Ok(hwnd) = window.hwnd() {
                set_window_position(hwnd, snapshot.normal.left, snapshot.normal.top);
            }
        }
    }
    #[cfg(not(windows))]
    let _ = window;
}

#[cfg(windows)]
fn auto_hide_loop(app: AppHandle, state: std::sync::Arc<std::sync::Mutex<AutoHideState>>) {
    loop {
        std::thread::sleep(POLL_INTERVAL);
        let (enabled, edges, overlay, reduce_motion) = {
            let app_state = app.state::<AppState>();
            let runtime = app_state
                .runtime
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            (
                runtime.settings.edge_autohide,
                runtime.settings.edge_autohide_edges.clone(),
                runtime.overlay,
                runtime.settings.reduce_motion,
            )
        };
        let Some(window) = app.get_window(WINDOW_LABEL) else {
            continue;
        };
        let Ok(hwnd) = window.hwnd() else {
            continue;
        };
        if !is_visible(hwnd) {
            continue;
        }

        let hidden = {
            let state = state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            state.hidden.clone()
        };
        if let Some(snapshot) = hidden {
            if !enabled || !snapshot.edge.is_enabled(&edges) {
                restore_snapshot(hwnd, &snapshot, &state, !reduce_motion);
                continue;
            }
            if let Some(cursor) = cursor_position() {
                if snapshot.edge.reveals(cursor, &snapshot.normal, &snapshot.work) {
                    restore_snapshot(hwnd, &snapshot, &state, !reduce_motion);
                }
            }
            continue;
        }

        if !enabled || overlay {
            clear_outside_timer(&state);
            continue;
        }
        if is_minimized(hwnd) || is_maximized(hwnd) {
            clear_outside_timer(&state);
            continue;
        }
        let Some(work) = work_area(hwnd) else {
            clear_outside_timer(&state);
            continue;
        };
        let Some(rect) = window_rect(hwnd) else {
            clear_outside_timer(&state);
            continue;
        };
        let Some(edge) = detect_edge(&rect, &work, &edges) else {
            clear_outside_timer(&state);
            continue;
        };
        let Some(cursor) = cursor_position() else {
            continue;
        };
        if rect.contains(cursor) || left_button_down() {
            clear_outside_timer(&state);
            continue;
        }

        let now = Instant::now();
        let should_hide = {
            let mut state = state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            let since = *state.outside_since.get_or_insert(now);
            now.duration_since(since) >= HIDE_DELAY
        };
        if !should_hide {
            continue;
        }

        let (x, y) = edge.hide_position(&rect, &work);
        let snapshot = HiddenSnapshot {
            edge,
            normal: edge.snapped_to_work(&rect, &work),
            work,
        };
        if reduce_motion {
            set_window_position(hwnd, x, y);
        } else {
            animate_window_position(hwnd, rect.left, rect.top, x, y);
        }
        {
            let mut state = state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            state.hidden = Some(snapshot);
            state.outside_since = None;
        }
        crate::logging::info(&format!("窗口已靠边隐藏（{}）。", edge.as_str()));
    }
}

#[cfg(windows)]
fn restore_snapshot(
    hwnd: windows::Win32::Foundation::HWND,
    snapshot: &HiddenSnapshot,
    state: &std::sync::Arc<std::sync::Mutex<AutoHideState>>,
    animate: bool,
) {
    if animate {
        let from = window_rect(hwnd).unwrap_or(snapshot.normal);
        animate_window_position(
            hwnd,
            from.left,
            from.top,
            snapshot.normal.left,
            snapshot.normal.top,
        );
    } else {
        set_window_position(hwnd, snapshot.normal.left, snapshot.normal.top);
    }
    let mut state = state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    state.hidden = None;
    state.outside_since = None;
    drop(state);
    crate::logging::info("窗口已从屏幕边缘恢复。");
}

#[cfg(windows)]
fn clear_outside_timer(state: &std::sync::Arc<std::sync::Mutex<AutoHideState>>) {
    let mut state = state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    state.outside_since = None;
}

#[cfg(windows)]
fn detect_edge(rect: &ScreenRect, work: &ScreenRect, edges: &[String]) -> Option<Edge> {
    // A window that is fully outside the monitor is not a docked window; a
    // window that protrudes past an edge still counts and will be snapped back
    // flush on reveal.
    let intersects = rect.left < work.right
        && rect.right > work.left
        && rect.top < work.bottom
        && rect.bottom > work.top;
    if !intersects {
        return None;
    }

    let mut best: Option<(Edge, i32)> = None;
    for edge in [Edge::Top, Edge::Left, Edge::Right] {
        if !edge.is_enabled(edges) {
            continue;
        }
        let (qualified, distance) = match edge {
            Edge::Top => (
                rect.top <= work.top + DOCK_DISTANCE,
                (rect.top - work.top).max(0),
            ),
            Edge::Left => (
                rect.left <= work.left + DOCK_DISTANCE,
                (rect.left - work.left).max(0),
            ),
            Edge::Right => (
                rect.right >= work.right - DOCK_DISTANCE,
                (work.right - rect.right).max(0),
            ),
        };
        if !qualified {
            continue;
        }
        if best.map_or(true, |(_, current)| distance < current) {
            best = Some((edge, distance));
        }
    }
    best.map(|(edge, _)| edge)
}

#[cfg(windows)]
fn window_rect(hwnd: windows::Win32::Foundation::HWND) -> Option<ScreenRect> {
    use windows::Win32::Foundation::RECT;
    use windows::Win32::UI::WindowsAndMessaging::GetWindowRect;

    let mut rect = RECT::default();
    unsafe { GetWindowRect(hwnd, &mut rect).ok()? };
    Some(ScreenRect {
        left: rect.left,
        top: rect.top,
        right: rect.right,
        bottom: rect.bottom,
    })
}

#[cfg(windows)]
fn work_area(hwnd: windows::Win32::Foundation::HWND) -> Option<ScreenRect> {
    use windows::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST,
    };

    let monitor = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) };
    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    if unsafe { GetMonitorInfoW(monitor, &mut info) }.as_bool() {
        Some(ScreenRect {
            left: info.rcWork.left,
            top: info.rcWork.top,
            right: info.rcWork.right,
            bottom: info.rcWork.bottom,
        })
    } else {
        None
    }
}

#[cfg(windows)]
fn cursor_position() -> Option<Point> {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;

    let mut point = POINT::default();
    unsafe { GetCursorPos(&mut point).ok()? };
    Some(Point {
        x: point.x,
        y: point.y,
    })
}

#[cfg(windows)]
fn left_button_down() -> bool {
    use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON};

    let state = unsafe { GetAsyncKeyState(VK_LBUTTON.0 as i32) };
    (state as u16 & 0x8000) != 0
}

#[cfg(windows)]
fn is_visible(hwnd: windows::Win32::Foundation::HWND) -> bool {
    use windows::Win32::UI::WindowsAndMessaging::IsWindowVisible;
    unsafe { IsWindowVisible(hwnd).as_bool() }
}

#[cfg(windows)]
fn is_minimized(hwnd: windows::Win32::Foundation::HWND) -> bool {
    use windows::Win32::UI::WindowsAndMessaging::IsIconic;
    unsafe { IsIconic(hwnd).as_bool() }
}

#[cfg(windows)]
fn is_maximized(hwnd: windows::Win32::Foundation::HWND) -> bool {
    use windows::Win32::UI::WindowsAndMessaging::IsZoomed;
    unsafe { IsZoomed(hwnd).as_bool() }
}

#[cfg(windows)]
fn set_window_position(hwnd: windows::Win32::Foundation::HWND, x: i32, y: i32) {
    use windows::Win32::UI::WindowsAndMessaging::{
        SetWindowPos, SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER,
    };
    unsafe {
        let _ = SetWindowPos(
            hwnd,
            None,
            x,
            y,
            0,
            0,
            SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
        );
    }
}

#[cfg(windows)]
fn animate_window_position(
    hwnd: windows::Win32::Foundation::HWND,
    from_x: i32,
    from_y: i32,
    to_x: i32,
    to_y: i32,
) {
    for step in 1..=SLIDE_STEPS {
        let progress = step as f64 / SLIDE_STEPS as f64;
        // Ease-out cubic: fast initial movement, gentle arrival.
        let eased = 1.0 - (1.0 - progress).powi(3);
        let x = from_x + ((to_x - from_x) as f64 * eased).round() as i32;
        let y = from_y + ((to_y - from_y) as f64 * eased).round() as i32;
        set_window_position(hwnd, x, y);
        if step < SLIDE_STEPS {
            std::thread::sleep(SLIDE_STEP_DELAY);
        }
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    fn rect(left: i32, top: i32, right: i32, bottom: i32) -> ScreenRect {
        ScreenRect {
            left,
            top,
            right,
            bottom,
        }
    }

    #[test]
    fn detects_a_window_docked_to_the_selected_edge() {
        let work = rect(0, 0, 1920, 1040);
        let window = rect(4, 200, 964, 800);
        let edges = ["left".to_string()];
        assert_eq!(detect_edge(&window, &work, &edges), Some(Edge::Left));
    }

    #[test]
    fn ignores_an_edge_that_is_not_selected() {
        let work = rect(0, 0, 1920, 1040);
        let window = rect(4, 200, 964, 800);
        let edges = ["right".to_string()];
        assert_eq!(detect_edge(&window, &work, &edges), None);
    }

    #[test]
    fn detects_a_window_that_protrudes_past_the_edge() {
        let work = rect(0, 0, 1920, 1040);
        let window = rect(-80, 200, 894, 808);
        let edges = ["left".to_string()];
        assert_eq!(detect_edge(&window, &work, &edges), Some(Edge::Left));
        assert_eq!(
            Edge::Left.snapped_to_work(&window, &work).left,
            work.left
        );
    }

    #[test]
    fn ignores_a_window_that_does_not_intersect_the_monitor() {
        let work = rect(0, 0, 1920, 1040);
        let window = rect(-1000, 200, -100, 808);
        let edges = ["left".to_string()];
        assert_eq!(detect_edge(&window, &work, &edges), None);
    }

    #[test]
    fn hidden_left_window_keeps_a_peek_strip() {
        let work = rect(0, 0, 1920, 1040);
        let window = rect(0, 200, 974, 808);
        assert_eq!(
            Edge::Left.hide_position(&window, &work),
            (-974 + EDGE_PEEK, 200)
        );
    }

    #[test]
    fn cursor_near_the_left_edge_reveals_the_window() {
        let work = rect(0, 0, 1920, 1040);
        let window = rect(0, 200, 974, 808);
        assert!(Edge::Left.reveals(
            Point { x: 40, y: 400 },
            &window,
            &work
        ));
        assert!(!Edge::Left.reveals(
            Point { x: 800, y: 400 },
            &window,
            &work
        ));
    }
}
