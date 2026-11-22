//! VRChat chatbox panel.
//!
//! The remote console reports a compact status snapshot through the remote
//! WebView's message channel. A background thread turns that snapshot into a
//! small ASCII panel and sends it to VRChat's `/chatbox/input` OSC endpoint.
//! The panel has one decorated title rule and no closing rule; most modes use
//! at most three text lines, while the performance breakdown uses four
//! (CPU model, CPU + RAM, GPU model, GPU + VRAM).

use std::time::{Duration, Instant};

use serde::Deserialize;
use tauri::{AppHandle, Manager};

use crate::osc;
use crate::state::AppState;

/// VRChat recommends refreshing the chatbox roughly every 1.5 seconds.
const SEND_INTERVAL: Duration = Duration::from_millis(1500);
/// A snapshot older than this means the remote page stopped reporting.
const STATUS_MAX_AGE: Duration = Duration::from_secs(6);
/// Repeat the "waiting" panel slowly so it does not flood the chatbox.
const WAITING_RESEND: Duration = Duration::from_secs(8);
/// Base width of the decorated title rule. VRChat centres every chatbox line,
/// so the flourish on both sides of the title stays balanced.
const RULE_BASE_WIDTH: usize = 30;
/// Upper bound for one text line. The rules and text lines are centred
/// independently, so the panel does not need a side rail or padding.
const CONTENT_LIMIT: usize = 27;
/// Hardware icons verified to render in VRChat's chatbox font.
const CPU_ICON: &str = "■";
const GPU_ICON: &str = "▤";
/// The complete message must stay within VRChat's chatbox character limit.
const MESSAGE_LIMIT: usize = osc::CHATBOX_LIMIT;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct RemoteStatus {
    pub date: String,
    pub weekday: String,
    pub time: String,
    pub weather: WeatherStatus,
    pub music: MusicStatus,
    pub system: SystemStatus,
    pub agent: AgentStatus,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct WeatherStatus {
    pub loaded: bool,
    pub available: bool,
    pub location: String,
    pub code: Option<i64>,
    pub is_day: Option<bool>,
    pub temperature: Option<f64>,
    pub high: Option<f64>,
    pub low: Option<f64>,
    /// `C` or `F`; the remote page already converted the values.
    pub unit: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct MusicStatus {
    pub has: bool,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub playing: bool,
    pub position: Option<f64>,
    pub duration: Option<f64>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SystemStatus {
    pub cpu_model: String,
    pub cpu_usage: Option<f64>,
    pub mem_used: Option<f64>,
    pub mem_total: Option<f64>,
    pub mem_percent: Option<f64>,
    pub gpu_ok: bool,
    pub gpu_name: String,
    pub gpu_usage: Option<f64>,
    pub vram_used: Option<f64>,
    pub vram_total: Option<f64>,
    pub vram_percent: Option<f64>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct AgentStatus {
    pub running: u32,
    pub waiting: u32,
    pub errors: u32,
    pub account_loaded: bool,
    pub signed_in: bool,
    pub low_balance: bool,
    pub balance: Option<f64>,
    pub currency: String,
    pub today_spend: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct StatusSnapshot {
    pub at: Instant,
    pub status: RemoteStatus,
}

/// Store one status report from the remote page. Malformed reports are
/// ignored: the next snapshot arrives one second later.
pub fn handle_status_message(app: &AppHandle, message: &serde_json::Value) {
    let Some(status) = message.get("status") else {
        return;
    };
    let Ok(status) = serde_json::from_value::<RemoteStatus>(status.clone()) else {
        return;
    };
    let state = app.state::<AppState>();
    let mut slot = state
        .remote_status
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    *slot = Some(StatusSnapshot {
        at: Instant::now(),
        status,
    });
}

/// Start the OSC refresh loop. The thread idles while the feature is disabled
/// or while no remote page is open, so settings can be changed at any time.
pub fn start(app: AppHandle) {
    std::thread::Builder::new()
        .name("vrchat-osc".to_string())
        .spawn(move || run(app))
        .ok();
}

fn run(app: AppHandle) {
    let mut sender = osc::UdpSender::new();
    let mut waiting_since: Option<Instant> = None;
    let mut last_error: Option<(Instant, String)> = None;

    loop {
        let (settings, remote_open) = {
            let state = app.state::<AppState>();
            let runtime = state
                .runtime
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            (runtime.settings.clone(), runtime.remote_url.is_some())
        };

        let custom_mode = settings
            .vrchat_osc_mode
            .trim()
            .eq_ignore_ascii_case("custom");
        if !settings.vrchat_osc_enabled || (!remote_open && !custom_mode) {
            sender.reset();
            waiting_since = None;
            last_error = None;
            std::thread::sleep(Duration::from_millis(400));
            continue;
        }

        let snapshot = {
            let state = app.state::<AppState>();
            let slot = state
                .remote_status
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            slot.clone()
        };

        let (text, waiting) = if custom_mode {
            (
                format_panel(
                    &RemoteStatus::default(),
                    "custom",
                    &settings.language,
                    &settings.vrchat_osc_custom_text,
                ),
                false,
            )
        } else {
            match snapshot {
                Some(snapshot) if snapshot.at.elapsed() < STATUS_MAX_AGE => (
                    format_panel(
                        &snapshot.status,
                        &settings.vrchat_osc_mode,
                        &settings.language,
                        &settings.vrchat_osc_custom_text,
                    ),
                    false,
                ),
                _ => (waiting_panel(&settings.language), true),
            }
        };
        if text.trim().is_empty() {
            std::thread::sleep(SEND_INTERVAL);
            continue;
        }

        let now = Instant::now();
        let should_send = !waiting
            || waiting_since
                .map(|sent| now.duration_since(sent) >= WAITING_RESEND)
                .unwrap_or(true);
        if should_send {
            let message = osc::chatbox_message(&text);
            match sender.send(
                &settings.vrchat_osc_host,
                settings.vrchat_osc_port,
                &message,
            ) {
                Ok(()) => last_error = None,
                Err(error) => {
                    let should_log = last_error
                        .as_ref()
                        .map(|(at, previous)| {
                            previous != &error || at.elapsed() >= Duration::from_secs(30)
                        })
                        .unwrap_or(true);
                    if should_log {
                        crate::logging::warn(&format!("VRChat OSC: {error}"));
                        last_error = Some((now, error));
                    }
                }
            }
            if waiting {
                waiting_since = Some(now);
            }
        }

        std::thread::sleep(SEND_INTERVAL);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Lang {
    Zh,
    En,
}

impl Lang {
    fn from_setting(value: &str) -> Self {
        if value.eq_ignore_ascii_case("en") {
            Self::En
        } else {
            Self::Zh
        }
    }

    fn pick(self, zh: &'static str, en: &'static str) -> &'static str {
        match self {
            Self::Zh => zh,
            Self::En => en,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Clock,
    Music,
    System,
    Agent,
    Custom,
}

impl Mode {
    fn from_setting(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "music" => Self::Music,
            "system" => Self::System,
            "agent" => Self::Agent,
            "custom" => Self::Custom,
            _ => Self::Clock,
        }
    }

    fn title(self, lang: Lang) -> &'static str {
        match self {
            Self::Clock => lang.pick("时间天气", "CLOCK"),
            Self::Music => lang.pick("正在播放", "NOW PLAYING"),
            Self::System => lang.pick("系统", "SYSTEM"),
            Self::Agent => lang.pick("大肥鱼", "BIG FISH"),
            Self::Custom => lang.pick("自定义", "CUSTOM"),
        }
    }

    fn icon(self) -> &'static str {
        match self {
            Self::Clock => "⏱",
            Self::Music => "♪",
            Self::System => "⚙",
            Self::Agent => "★",
            Self::Custom => "☺",
        }
    }

    fn heading(self, lang: Lang) -> String {
        format!("{} {}", self.icon(), self.title(lang))
    }
}

pub fn format_panel(
    status: &RemoteStatus,
    mode: &str,
    language: &str,
    custom_text: &str,
) -> String {
    let lang = Lang::from_setting(language);
    match Mode::from_setting(mode) {
        Mode::Clock => clock_panel(status, lang),
        Mode::Music => music_panel(status, lang),
        Mode::System => system_panel(status, lang),
        Mode::Agent => agent_panel(status, lang),
        Mode::Custom => crate::settings::sanitize_custom_text(custom_text),
    }
}

fn waiting_panel(language: &str) -> String {
    let lang = Lang::from_setting(language);
    render_panel(
        &format!("◌ {}", lang.pick("远控伴侣", "COMPANION")),
        vec![
            lang.pick("等待远控页面", "Waiting for the remote page")
                .to_string(),
            lang.pick("连接后显示状态", "Status appears after connecting")
                .to_string(),
        ],
    )
}

fn clock_panel(status: &RemoteStatus, lang: Lang) -> String {
    let heading = Mode::Clock.heading(lang);
    let time = clock_value(&status.time);
    let weekday = value_or(&status.weekday, "");
    let date = value_or(&status.date, "");
    let mut first = vec![time];
    if !weekday.is_empty() {
        first.push(weekday.to_string());
    }
    if !date.is_empty() {
        first.push(date.to_string());
    }
    let mut lines = vec![first.join(" · ")];

    if !status.weather.loaded {
        lines.push(lang.pick("天气同步中", "Weather syncing").to_string());
        return render_panel(&heading, lines);
    }
    if !status.weather.available {
        lines.push(lang.pick("天气不可用", "Weather unavailable").to_string());
        return render_panel(&heading, lines);
    }

    let condition = format!(
        "{} {}",
        weather_icon(status.weather.code, status.weather.is_day),
        weather_label(status.weather.code, lang)
    );
    let location = status.weather.location.trim();
    lines.push(if location.is_empty() {
        condition
    } else {
        format!("{location} · {condition}")
    });

    let unit = unit_suffix(&status.weather.unit);
    let mut third = vec![format_temperature(status.weather.temperature, unit)];
    if let Some(high) = status.weather.high {
        third.push(match status.weather.low {
            Some(low) => format!(
                "{}/{}",
                format_degrees(Some(high)),
                format_degrees(Some(low))
            ),
            None => format_degrees(Some(high)),
        });
    }
    lines.push(third.join(" · "));

    render_panel(&heading, lines)
}

fn music_panel(status: &RemoteStatus, lang: Lang) -> String {
    let heading = Mode::Music.heading(lang);
    if !status.music.has {
        return render_panel(
            &heading,
            vec![
                lang.pick("没有正在播放的音乐", "Nothing playing")
                    .to_string(),
                lang.pick("打开播放器后显示", "Play something to show it here")
                    .to_string(),
            ],
        );
    }

    let song = if status.music.title.trim().is_empty() {
        lang.pick("未知曲目", "Unknown track").to_string()
    } else {
        status.music.title.trim().to_string()
    };
    let artist = status.music.artist.trim();
    let album = status.music.album.trim();
    let second = match (artist.is_empty(), album.is_empty()) {
        (false, _) => artist.to_string(),
        (true, false) => album.to_string(),
        (true, true) => lang.pick("未知艺术家", "Unknown artist").to_string(),
    };

    render_panel(&heading, vec![song, second, progress_line(status)])
}

fn progress_line(status: &RemoteStatus) -> String {
    let position = status.music.position.unwrap_or(0.0).max(0.0);
    let duration = status.music.duration.unwrap_or(0.0).max(0.0);
    let times = format!(
        "{}/{}",
        format_duration(Some(position)),
        if duration > 0.0 {
            format_duration(Some(duration))
        } else {
            "--:--".to_string()
        }
    );
    // The bar takes whatever is left of the 27-column line after the mark.
    let mark = if status.music.playing { "▶" } else { "⏸" };
    let width = (22 - display_width(mark) - display_width(&times)).clamp(8, 12);
    let bar = progress_bar(position, duration, status.music.playing, width);
    format!("{mark} {bar} {times}")
}

fn system_panel(status: &RemoteStatus, lang: Lang) -> String {
    let heading = Mode::System.heading(lang);
    let cpu_model = short_model(&status.system.cpu_model, 21);
    let cpu_name = if cpu_model.is_empty() {
        "--"
    } else {
        cpu_model.as_str()
    };
    let cpu_line = format!("{CPU_ICON} CPU {cpu_name}");
    let cpu_stats = format!(
        "{} · {}",
        format_percent(status.system.cpu_usage),
        memory_stat(
            "RAM",
            status.system.mem_percent,
            status.system.mem_used,
            status.system.mem_total
        )
    );

    let gpu_model = short_model(&status.system.gpu_name, 21);
    let gpu_name = if gpu_model.is_empty() {
        "--"
    } else {
        gpu_model.as_str()
    };
    let mut lines = vec![cpu_line, cpu_stats];
    if status.system.gpu_ok {
        lines.push(format!("{GPU_ICON} GPU {gpu_name}"));
        lines.push(format!(
            "{} · {}",
            format_percent(status.system.gpu_usage),
            memory_stat(
                "VRAM",
                status.system.vram_percent,
                status.system.vram_used,
                status.system.vram_total
            )
        ));
    } else {
        lines.push(format!(
            "{GPU_ICON} {}",
            lang.pick("GPU 不可用", "GPU unavailable")
        ));
    }

    render_panel_with_limit(&heading, lines, 4)
}

fn memory_stat(label: &str, percent: Option<f64>, used: Option<f64>, total: Option<f64>) -> String {
    let percent = format_percent(percent);
    let capacity = format_capacity(used, total);
    if capacity == "--" {
        format!("{label} {percent}")
    } else {
        format!("{label} {percent} {capacity}")
    }
}

fn agent_panel(status: &RemoteStatus, lang: Lang) -> String {
    let heading = Mode::Agent.heading(lang);
    let agent = &status.agent;
    let busy = agent.running > 0 || agent.waiting > 0;
    let mut details = Vec::new();
    if agent.running > 0 {
        details.push(match lang {
            Lang::Zh => format!("运行{}", agent.running),
            Lang::En => format!("{} running", agent.running),
        });
    }
    if agent.waiting > 0 {
        details.push(match lang {
            Lang::Zh => format!("等待{}", agent.waiting),
            Lang::En => format!("{} waiting", agent.waiting),
        });
    }
    if agent.errors > 0 {
        details.push(match lang {
            Lang::Zh => format!("错误{}", agent.errors),
            Lang::En => format!("{} error", agent.errors),
        });
    }
    let (icon, state) = if busy {
        ("●", lang.pick("工作中", "Working"))
    } else {
        ("○", lang.pick("闲置", "Idle"))
    };
    let mut first = format!("{icon} {state}");
    for detail in details {
        let candidate = format!("{first} · {detail}");
        if display_width(&candidate) <= CONTENT_LIMIT {
            first = candidate;
        }
    }

    let symbol = currency_symbol(&agent.currency);
    let mut second = if !agent.account_loaded {
        format!(
            "{} {}",
            lang.pick("余额", "Balance"),
            lang.pick("同步中", "syncing")
        )
    } else if !agent.signed_in {
        format!(
            "{} {}",
            lang.pick("余额", "Balance"),
            lang.pick("未登录", "signed out")
        )
    } else if let Some(balance) = agent.balance {
        format!(
            "{} {}{}",
            lang.pick("余额", "Balance"),
            symbol,
            format_money(balance)
        )
    } else {
        format!("{} --", lang.pick("余额", "Balance"))
    };
    if agent.low_balance {
        second.push_str(lang.pick(" · 偏低", " · low"));
    }

    let mut lines = vec![first, second];
    if let Some(today) = agent.today_spend {
        lines.push(format!(
            "{} {}{}",
            lang.pick("今日消费", "Today"),
            symbol,
            format_money(today)
        ));
    }

    render_panel(&heading, lines)
}

fn weather_label(code: Option<i64>, lang: Lang) -> &'static str {
    let group = weather_group(code);
    match group {
        WeatherGroup::Clear => lang.pick("晴", "Clear"),
        WeatherGroup::Partly => lang.pick("多云", "Partly cloudy"),
        WeatherGroup::Cloudy => lang.pick("阴", "Cloudy"),
        WeatherGroup::Fog => lang.pick("雾", "Fog"),
        WeatherGroup::Rain => lang.pick("雨", "Rain"),
        WeatherGroup::HeavyRain => lang.pick("大雨", "Heavy rain"),
        WeatherGroup::Snow => lang.pick("雪", "Snow"),
        WeatherGroup::Storm => lang.pick("雷雨", "Thunderstorm"),
        WeatherGroup::Unknown => lang.pick("天气", "Weather"),
    }
}

fn weather_icon(code: Option<i64>, is_day: Option<bool>) -> &'static str {
    match weather_group(code) {
        WeatherGroup::Clear => {
            if is_day == Some(false) {
                "●"
            } else {
                "☀"
            }
        }
        WeatherGroup::Partly | WeatherGroup::Cloudy | WeatherGroup::Unknown => "☁",
        WeatherGroup::Fog => "☁",
        WeatherGroup::Rain | WeatherGroup::HeavyRain => "☂",
        WeatherGroup::Snow => "❄",
        WeatherGroup::Storm => "⚡",
    }
}

fn weather_group(code: Option<i64>) -> WeatherGroup {
    match code {
        Some(0) | Some(1) => WeatherGroup::Clear,
        Some(2) => WeatherGroup::Partly,
        Some(3) => WeatherGroup::Cloudy,
        Some(45) | Some(48) => WeatherGroup::Fog,
        Some(51) | Some(53) | Some(55) | Some(56) | Some(57) | Some(61) | Some(66) | Some(67)
        | Some(80) | Some(81) => WeatherGroup::Rain,
        Some(65) | Some(82) => WeatherGroup::HeavyRain,
        Some(71) | Some(73) | Some(75) | Some(77) | Some(85) | Some(86) => WeatherGroup::Snow,
        Some(95) | Some(96) | Some(99) => WeatherGroup::Storm,
        _ => WeatherGroup::Unknown,
    }
}

enum WeatherGroup {
    Clear,
    Partly,
    Cloudy,
    Fog,
    Rain,
    HeavyRain,
    Snow,
    Storm,
    Unknown,
}

fn render_panel(title: &str, lines: Vec<String>) -> String {
    render_panel_with_limit(title, lines, 3)
}

fn render_panel_with_limit(title: &str, lines: Vec<String>, max_lines: usize) -> String {
    let lines: Vec<String> = lines
        .into_iter()
        .take(max_lines)
        .map(|line| truncate_display(&clean_line(&line), CONTENT_LIMIT))
        .filter(|line| !line.is_empty())
        .collect();

    let text = build_panel(title, &lines);
    if text.chars().count() <= MESSAGE_LIMIT {
        return text;
    }
    // The fixed panel width keeps normal panels well below the chatbox limit;
    // this only guards against a title that is longer than the rule.
    text.chars().take(MESSAGE_LIMIT).collect()
}

fn build_panel(title: &str, lines: &[String]) -> String {
    let width = rule_width(title);
    let mut text = top_rule(title, width);
    for line in lines {
        text.push('\n');
        text.push_str(line);
    }
    text
}

/// Keep an even number of flourish columns around the title so both sides match.
fn rule_width(title: &str) -> usize {
    if display_width(title) % 2 == 0 {
        RULE_BASE_WIDTH
    } else {
        RULE_BASE_WIDTH + 1
    }
}

/// `•·=·=·=« ⏱ 时间天气 »·=·=·=·•`
fn top_rule(title: &str, width: usize) -> String {
    let title_width = display_width(title);
    let inner = width.saturating_sub(title_width + 6);
    let left = inner / 2;
    let left_pattern = flourish(left);
    let right_pattern: String = left_pattern.chars().rev().collect();
    format!("•{left_pattern}« {title} »{right_pattern}•")
}

/// Alternating dot and dash pattern used on both sides of the title.
fn flourish(width: usize) -> String {
    (0..width)
        .map(|index| if index % 2 == 0 { '·' } else { '=' })
        .collect()
}

fn clean_line(value: &str) -> String {
    value
        .chars()
        .map(|ch| if ch.is_control() { ' ' } else { ch })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn truncate_display(value: &str, max: usize) -> String {
    if max < 4 {
        return String::new();
    }
    if display_width(value) <= max {
        return value.to_string();
    }
    let budget = max - 2;
    let mut result = String::new();
    let mut width = 0;
    for ch in value.chars() {
        let char_width = display_width_of(ch);
        if width + char_width > budget {
            break;
        }
        result.push(ch);
        width += char_width;
    }
    while result.ends_with(' ') {
        result.pop();
    }
    result.push_str("..");
    result
}

fn display_width(value: &str) -> usize {
    value.chars().map(display_width_of).sum()
}

/// CJK and other full-width characters occupy two columns in the chatbox;
/// everything else is treated as one column.
fn display_width_of(ch: char) -> usize {
    let code = ch as u32;
    if matches!(
        code,
        0x1100..=0x115F
            | 0x2E80..=0x303E
            | 0x3041..=0x33FF
            | 0x3400..=0x4DBF
            | 0x4E00..=0x9FFF
            | 0xA000..=0xA4CF
            | 0xAC00..=0xD7A3
            | 0xF900..=0xFAFF
            | 0xFE30..=0xFE4F
            | 0xFF00..=0xFF60
            | 0xFFE0..=0xFFE6
    ) {
        2
    } else {
        1
    }
}

/// The chatbox panel shows minutes only; strip any seconds a page may report.
fn clock_value(value: &str) -> String {
    let trimmed = value.trim();
    let mut parts = trimmed.split(':');
    match (parts.next(), parts.next()) {
        (Some(hour), Some(minute)) if !hour.is_empty() && !minute.is_empty() => {
            format!("{hour}:{minute}")
        }
        _ => {
            if trimmed.is_empty() {
                "--:--".to_string()
            } else {
                trimmed.to_string()
            }
        }
    }
}

fn value_or<'a>(value: &'a str, fallback: &'a str) -> &'a str {
    if value.trim().is_empty() {
        fallback
    } else {
        value.trim()
    }
}

fn unit_suffix(unit: &str) -> &'static str {
    if unit.eq_ignore_ascii_case("f") {
        "°F"
    } else {
        "°C"
    }
}

fn format_temperature(value: Option<f64>, unit: &str) -> String {
    match value {
        Some(number) if number.is_finite() => format!("{}{unit}", number.round() as i64),
        _ => "--".to_string(),
    }
}

fn format_degrees(value: Option<f64>) -> String {
    match value {
        Some(number) if number.is_finite() => format!("{}°", number.round() as i64),
        _ => "--".to_string(),
    }
}

fn format_percent(value: Option<f64>) -> String {
    match value {
        Some(number) if number.is_finite() => {
            format!("{}%", number.clamp(0.0, 100.0).round() as i64)
        }
        _ => "--".to_string(),
    }
}

fn format_capacity(used: Option<f64>, total: Option<f64>) -> String {
    let Some(total) = total.filter(|value| value.is_finite() && *value > 0.0) else {
        return "--".to_string();
    };
    let total = total / 1024.0 / 1024.0 / 1024.0;
    match used.filter(|value| value.is_finite() && *value >= 0.0) {
        Some(used) => format!(
            "{}/{}G",
            (used / 1024.0 / 1024.0 / 1024.0).round() as i64,
            total.round() as i64
        ),
        None => format!("--/{}G", total.round() as i64),
    }
}

fn format_duration(seconds: Option<f64>) -> String {
    match seconds {
        Some(value) if value.is_finite() && value >= 0.0 => {
            let total = value.round() as u64;
            format!("{}:{:02}", total / 60, total % 60)
        }
        _ => "--:--".to_string(),
    }
}

fn progress_bar(position: f64, duration: f64, playing: bool, width: usize) -> String {
    if duration <= 0.0 {
        return format!("[{}]", "-".repeat(width));
    }
    let fraction = (position / duration).clamp(0.0, 1.0);
    let filled = ((fraction * width as f64).floor() as usize).min(width);
    let bar = if playing {
        let done = filled.min(width.saturating_sub(1));
        format!(
            "{}{}{}",
            "=".repeat(done),
            '>',
            "-".repeat(width - done - 1)
        )
    } else {
        format!("{}{}", "=".repeat(filled), "-".repeat(width - filled))
    };
    format!("[{bar}]")
}

fn short_model(raw: &str, limit: usize) -> String {
    let cleaned = raw
        .replace("(R)", " ")
        .replace("(TM)", " ")
        .replace("(r)", " ")
        .replace("(tm)", " ");
    let tokens: Vec<&str> = cleaned
        .split_whitespace()
        .take_while(|token| !token.eq_ignore_ascii_case("with"))
        .filter(|token| !is_model_filler(token))
        .collect();
    if tokens.is_empty() {
        return String::new();
    }
    let Some(model_index) = tokens
        .iter()
        .rposition(|token| token.chars().any(|ch| ch.is_ascii_digit()) && !is_model_suffix(token))
    else {
        return truncate_display(&tokens.join(" "), limit);
    };

    // Keep short suffixes such as "XT" or "Ti" that belong to the model name,
    // but ignore "8-Core" / "16-Thread" style counters.
    let mut end = model_index;
    for index in model_index + 1..tokens.len() {
        if is_model_suffix(tokens[index]) {
            continue;
        }
        let candidate = tokens[model_index..=index].join(" ");
        if display_width(&candidate) > limit {
            break;
        }
        end = index;
    }

    let mut model = tokens[model_index..=end].join(" ");
    for index in (0..model_index).rev() {
        let candidate = format!("{} {}", tokens[index], model);
        if display_width(&candidate) > limit {
            break;
        }
        model = candidate;
    }
    truncate_display(&model, limit)
}

fn is_model_filler(token: &str) -> bool {
    matches!(
        token.to_ascii_lowercase().as_str(),
        "amd"
            | "intel"
            | "nvidia"
            | "geforce"
            | "core"
            | "processor"
            | "cpu"
            | "gpu"
            | "graphics"
            | "arc"
    )
}

/// "8-Core", "16-Thread" and similar suffixes follow the model name and would
/// otherwise win the "last token with a digit" search.
fn is_model_suffix(token: &str) -> bool {
    let lower = token.to_ascii_lowercase();
    lower.ends_with("core")
        || lower.ends_with("threads")
        || lower.ends_with("thread")
        || lower.ends_with("processor")
        || lower.ends_with("edition")
}

fn currency_symbol(currency: &str) -> String {
    match currency.trim().to_ascii_uppercase().as_str() {
        "" | "CNY" | "RMB" | "JPY" => "¥".to_string(),
        "USD" => "$".to_string(),
        "EUR" => "€".to_string(),
        other => format!("{other} "),
    }
}

fn format_money(value: f64) -> String {
    if !value.is_finite() {
        return "--".to_string();
    }
    let text = format!("{value:.2}");
    text.trim_end_matches('0').trim_end_matches('.').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_status() -> RemoteStatus {
        serde_json::from_value(serde_json::json!({
            "date": "11-23",
            "weekday": "周日",
            "time": "02:51:47",
            "weather": {
                "loaded": true,
                "available": true,
                "location": "上海",
                "code": 2,
                "isDay": false,
                "temperature": 18,
                "feelsLike": 17,
                "high": 21,
                "low": 14,
                "unit": "C"
            },
            "music": {
                "has": true,
                "title": "标准外",
                "artist": "Arboi氩男孩",
                "album": "Some Album",
                "playing": true,
                "position": 83,
                "duration": 220
            },
            "system": {
                "cpuModel": "13th Gen Intel(R) Core(TM) i7-13700K",
                "cpuUsage": 42.4,
                "memUsed": 13743895347.2,
                "memTotal": 34359738368.0,
                "memPercent": 39.1,
                "gpuOk": true,
                "gpuName": "NVIDIA GeForce RTX 3060",
                "gpuUsage": 61.0,
                "vramUsed": 4509715660.8,
                "vramTotal": 8589934592.0,
                "vramPercent": 52.4
            },
            "agent": {
                "running": 2,
                "waiting": 1,
                "errors": 0,
                "accountLoaded": true,
                "signedIn": true,
                "lowBalance": false,
                "balance": 12.34,
                "currency": "CNY",
                "todaySpend": 3.21
            }
        }))
        .expect("sample status")
    }

    fn assert_panel(text: &str) {
        assert!(text.chars().count() <= MESSAGE_LIMIT, "{text}");
        let lines: Vec<&str> = text.lines().collect();
        assert!(lines.len() <= 5, "{text}");
        assert!(lines.len() >= 2, "{text}");
        assert!(text.starts_with('•'), "{text}");
        // The panel is centred by VRChat: no side rails, no closing rule, and
        // every text line stays within the wrap limit.
        for line in lines.iter().skip(1) {
            assert!(!line.starts_with('|'), "{text}");
            assert!(display_width(line) <= CONTENT_LIMIT, "{text}");
        }
    }

    #[test]
    fn panels_keep_their_expected_text_line_count() {
        let status = sample_status();
        for mode in ["clock", "music", "system", "agent"] {
            for language in ["zh", "en"] {
                let text = format_panel(&status, mode, language, "");
                assert_panel(&text);
                let content = text.lines().count() - 1;
                let limit = if mode == "system" { 4 } else { 3 };
                assert!(content <= limit, "{text}");
                assert!(content >= 1, "{text}");
            }
        }
    }

    #[test]
    fn clock_panel_shows_time_weather_and_temperatures() {
        let text = format_panel(&sample_status(), "clock", "zh", "");
        assert!(text.contains("⏱ 时间天气"), "{text}");
        assert!(text.contains("02:51"), "{text}");
        assert!(!text.contains("02:51:47"), "{text}");
        assert!(text.contains("上海"), "{text}");
        assert!(text.contains("☁ 多云"), "{text}");
        assert!(text.contains("18°C · 21°/14°"), "{text}");
        assert_panel(&text);
    }

    #[test]
    fn music_panel_shows_icon_song_and_progress() {
        let text = format_panel(&sample_status(), "music", "zh", "");
        assert!(text.contains("♪ 正在播放"), "{text}");
        assert!(text.contains("标准外"), "{text}");
        assert!(text.contains("Arboi氩男孩"), "{text}");
        assert!(text.contains("1:23/3:40"), "{text}");
        assert_panel(&text);
    }

    #[test]
    fn system_panel_separates_models_and_groups_memory() {
        let text = format_panel(&sample_status(), "system", "en", "");
        assert!(text.contains("■ CPU 13th Gen i7-13700K"), "{text}");
        assert!(text.contains("42% · RAM 39% 13/32G"), "{text}");
        assert!(text.contains("▤ GPU RTX 3060"), "{text}");
        assert!(text.contains("61% · VRAM 52% 4/8G"), "{text}");
        assert_eq!(text.lines().count(), 5, "{text}");
        assert_panel(&text);
    }

    #[test]
    fn agent_panel_shows_state_balance_and_today_spend() {
        let text = format_panel(&sample_status(), "agent", "zh", "");
        assert!(text.contains("★ 大肥鱼"), "{text}");
        assert!(text.contains("● 工作中"), "{text}");
        assert!(text.contains("余额 ¥12.34"), "{text}");
        assert!(text.contains("今日消费 ¥3.21"), "{text}");
        assert_panel(&text);
    }

    #[test]
    fn panels_survive_extreme_values_within_the_character_limit() {
        let mut status = sample_status();
        status.music.title = "W".repeat(200);
        status.music.artist = "A".repeat(200);
        status.music.album = "B".repeat(200);
        status.weather.location = "L".repeat(200);
        status.system.cpu_model = "C".repeat(200);
        status.system.gpu_name = "G".repeat(200);
        for mode in ["clock", "music", "system", "agent"] {
            let text = format_panel(&status, mode, "en", "");
            assert_panel(&text);
        }
    }

    #[test]
    fn missing_status_degrades_to_placeholders() {
        let status = RemoteStatus::default();
        for mode in ["clock", "music", "system", "agent"] {
            let text = format_panel(&status, mode, "en", "");
            assert_panel(&text);
        }
    }

    #[test]
    fn waiting_panel_is_short_and_decorated() {
        let text = waiting_panel("zh");
        assert_panel(&text);
        assert!(text.contains("◌ 远控伴侣"), "{text}");
        assert!(text.contains("等待远控页面"), "{text}");
    }

    #[test]
    fn unknown_mode_falls_back_to_clock() {
        let status = sample_status();
        assert_eq!(
            format_panel(&status, "unknown", "en", ""),
            format_panel(&status, "clock", "en", "")
        );
    }

    #[test]
    fn cjk_and_ascii_widths_are_counted_per_column() {
        assert_eq!(display_width("abc"), 3);
        assert_eq!(display_width("大肥鱼"), 6);
        let truncated = truncate_display("大肥鱼状态良好", 6);
        assert_eq!(truncated, "大肥..");
        assert!(display_width(&truncated) <= 6);
    }

    #[test]
    fn progress_bar_grows_with_playback() {
        assert_eq!(progress_bar(0.0, 200.0, true, 10), "[>---------]");
        assert_eq!(progress_bar(100.0, 200.0, true, 10), "[=====>----]");
        assert_eq!(progress_bar(100.0, 200.0, false, 10), "[=====-----]");
        assert_eq!(progress_bar(0.0, 0.0, true, 10), "[----------]");
        assert_eq!(progress_bar(170.0, 173.0, false, 11), "[==========-]");
    }

    #[test]
    fn model_shortening_keeps_the_model_token() {
        assert_eq!(
            short_model("AMD Ryzen 7 5800X 8-Core Processor", 18),
            "Ryzen 7 5800X"
        );
        assert_eq!(
            short_model("13th Gen Intel(R) Core(TM) i7-13700K", 20),
            "13th Gen i7-13700K"
        );
        assert_eq!(short_model("NVIDIA GeForce RTX 3060", 17), "RTX 3060");
        assert_eq!(
            short_model("AMD Radeon RX 7800 XT", 17),
            "Radeon RX 7800 XT"
        );
    }

    #[test]
    fn clock_value_drops_seconds() {
        assert_eq!(clock_value("02:51:47"), "02:51");
        assert_eq!(clock_value("02:51"), "02:51");
        assert_eq!(clock_value(""), "--:--");
    }

    #[test]
    fn money_and_percent_formatting_is_compact() {
        assert_eq!(format_money(12.34), "12.34");
        assert_eq!(format_money(0.5), "0.5");
        assert_eq!(format_money(12.0), "12");
        assert_eq!(format_percent(Some(42.6)), "43%");
        assert_eq!(format_percent(None), "--");
        assert_eq!(currency_symbol("USD"), "$");
        assert_eq!(currency_symbol("CNY"), "¥");
    }

    #[test]
    fn title_rule_flourishes_are_balanced_around_the_title() {
        for title in ["⏱ 时间天气", "⚙ 系统", "★ 大肥鱼", "♪ NOW PLAYING"] {
            let width = rule_width(title);
            let top = top_rule(title, width);
            assert_eq!(display_width(&top), width, "{top}");
            assert!(top.starts_with('•') && top.ends_with('•'), "{top}");
            let frame = format!("« {title} »");
            let (left, right) = top.split_once(&frame).expect("title frame");
            assert_eq!(
                display_width(left.trim_start_matches('•')),
                display_width(right.trim_end_matches('•')),
                "{top}"
            );
        }
    }

    #[test]
    fn music_panel_switches_between_play_and_pause_marks() {
        let mut status = sample_status();
        status.music.playing = true;
        assert!(format_panel(&status, "music", "en", "").contains("▶ "));
        status.music.playing = false;
        assert!(format_panel(&status, "music", "en", "").contains("⏸ "));
    }

    #[test]
    fn custom_panel_is_raw_text_without_decoration() {
        let raw = "  第一行  \n第二行";
        let text = format_panel(&RemoteStatus::default(), "custom", "zh", raw);
        assert_eq!(text, raw);
        assert!(!text.contains('•'));
        assert!(!text.contains('«'));
    }

    #[test]
    fn status_json_shape_matches_the_remote_page_report() {
        let status = sample_status();
        assert_eq!(status.system.cpu_usage, Some(42.4));
        assert_eq!(status.agent.running, 2);
        assert_eq!(status.weather.unit, "C");
    }
}
