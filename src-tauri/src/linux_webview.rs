//! Linux desktop chrome.
//!
//! The Linux WebKit child views cannot be squeezed into a 44px WebView (their
//! GTK minimum height is much larger), so Linux uses a native two-row top area
//! instead of the Windows HTML titlebar:
//!
//!   app bar:  title + window controls
//!   toolbar:  settings / back to connection page / refresh
//!   remote:   the second WebView fills the rest of the window
//!
//! Settings and other local dialogs still use the local chrome WebView, which
//! is shown full-window while an overlay is open.

use gtk::prelude::*;
use tauri::{AppHandle, Emitter, Manager, Webview, Window};

const APP_BAR: &str = "dsh-linux-appbar";
const TOOLBAR: &str = "dsh-linux-toolbar";
const TOPBAR: &str = "dsh-linux-topbar";
const TITLE: &str = "dsh-linux-title";
const SETTINGS: &str = "dsh-linux-settings";
const BACK: &str = "dsh-linux-back";
const REFRESH: &str = "dsh-linux-refresh";
const ABOUT: &str = "dsh-linux-about";
const LANGUAGE: &str = "dsh-linux-language";

const CSS: &str = r#"
#dsh-linux-topbar {
  background: #0b0f16;
  border-bottom: 1px solid rgba(255, 255, 255, 0.08);
}
#dsh-linux-topbar.light {
  background: #eef2f8;
  border-bottom-color: rgba(15, 23, 42, 0.12);
}
#dsh-linux-appbar { min-height: 44px; padding: 0 8px; }
#dsh-linux-toolbar { min-height: 38px; padding: 0 10px 4px; }
#dsh-linux-topbar button {
  min-height: 26px;
  padding: 2px 10px;
  border: 0;
  border-radius: 6px;
  box-shadow: none;
  background: transparent;
  color: #dfe6f1;
}
#dsh-linux-topbar button:hover { background: rgba(255, 255, 255, 0.07); }
#dsh-linux-topbar.light button { color: #162033; }
#dsh-linux-topbar.light button:hover { background: rgba(15, 23, 42, 0.07); }
#dsh-linux-title { color: #edf1f8; font-weight: 600; }
#dsh-linux-topbar.light #dsh-linux-title { color: #172033; }
"#;

pub fn prepare(window: &Window, app: AppHandle) {
    let Ok(vbox) = window.default_vbox() else {
        return;
    };
    if find_by_name(&vbox, TOPBAR).is_some() {
        return;
    }
    vbox.set_orientation(gtk::Orientation::Vertical);
    vbox.set_homogeneous(false);
    vbox.set_spacing(0);

    let topbar = gtk::Box::new(gtk::Orientation::Vertical, 0);
    topbar.set_widget_name(TOPBAR);

    let appbar = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    appbar.set_widget_name(APP_BAR);
    let title = gtk::Label::new(Some("DSH 远控伴侣"));
    title.set_widget_name(TITLE);
    title.set_xalign(0.0);
    title.set_hexpand(true);
    title.set_margin_start(6);
    appbar.pack_start(&title, true, true, 0);

    let minimize = gtk::Button::with_label("—");
    minimize.set_tooltip_text(Some("最小化"));
    minimize.set_relief(gtk::ReliefStyle::None);
    let window_for_min = window.clone();
    minimize.connect_clicked(move |_| {
        let _ = window_for_min.minimize();
    });
    appbar.pack_start(&minimize, false, false, 0);

    let maximize = gtk::Button::with_label("□");
    maximize.set_tooltip_text(Some("最大化"));
    maximize.set_relief(gtk::ReliefStyle::None);
    let window_for_max = window.clone();
    maximize.connect_clicked(move |_| {
        if window_for_max.is_maximized().unwrap_or(false) {
            let _ = window_for_max.unmaximize();
        } else {
            let _ = window_for_max.maximize();
        }
    });
    appbar.pack_start(&maximize, false, false, 0);

    let close = gtk::Button::with_label("×");
    close.set_tooltip_text(Some("关闭"));
    close.set_relief(gtk::ReliefStyle::None);
    let window_for_close = window.clone();
    close.connect_clicked(move |_| {
        let _ = window_for_close.close();
    });
    appbar.pack_start(&close, false, false, 0);

    let window_for_drag = window.clone();
    appbar.connect_button_press_event(move |_, event| {
        if event.button() == 1 {
            let _ = window_for_drag.start_dragging();
        }
        gtk::glib::Propagation::Proceed
    });

    let toolbar = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    toolbar.set_widget_name(TOOLBAR);

    let settings = gtk::Button::with_label("设置");
    settings.set_widget_name(SETTINGS);
    settings.set_relief(gtk::ReliefStyle::None);
    let settings_app = app.clone();
    settings.connect_clicked(move |_| {
        let _ = settings_app.emit_to(crate::layout::CHROME_LABEL, "linux-open-settings", ());
    });
    toolbar.pack_start(&settings, false, false, 0);

    let back = gtk::Button::with_label("返回");
    back.set_widget_name(BACK);
    back.set_tooltip_text(Some("回到连接页"));
    back.set_relief(gtk::ReliefStyle::None);
    let back_app = app.clone();
    back.connect_clicked(move |_| {
        let _ = back_app.emit_to(crate::layout::CHROME_LABEL, "linux-back-home", ());
    });
    toolbar.pack_start(&back, false, false, 0);

    let refresh = gtk::Button::with_label("刷新");
    refresh.set_widget_name(REFRESH);
    refresh.set_relief(gtk::ReliefStyle::None);
    let refresh_app = app.clone();
    refresh.connect_clicked(move |_| {
        let _ = refresh_app.emit_to(crate::layout::CHROME_LABEL, "linux-refresh", ());
    });
    toolbar.pack_start(&refresh, false, false, 0);

    let about = gtk::Button::with_label("关于");
    about.set_widget_name(ABOUT);
    about.set_relief(gtk::ReliefStyle::None);
    let about_app = app.clone();
    about.connect_clicked(move |_| {
        let _ = about_app.emit_to(crate::layout::CHROME_LABEL, "linux-open-about", ());
    });
    toolbar.pack_start(&about, false, false, 0);

    let language = gtk::Button::with_label("中文");
    language.set_widget_name(LANGUAGE);
    language.set_relief(gtk::ReliefStyle::None);
    language.set_tooltip_text(Some("切换语言 / Switch language"));
    let language_app = app.clone();
    language.connect_clicked(move |_| {
        let next = {
            let state = language_app.state::<crate::state::AppState>();
            let runtime = state
                .runtime
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if runtime.settings.language.eq_ignore_ascii_case("en") {
                "zh"
            } else {
                "en"
            }
        };
        let _ = language_app.emit_to(crate::layout::CHROME_LABEL, "linux-set-language", next);
    });
    toolbar.pack_start(&language, false, false, 0);

    topbar.pack_start(&appbar, false, false, 0);
    topbar.pack_start(&toolbar, false, false, 0);
    vbox.pack_start(&topbar, false, false, 0);
    topbar.show_all();

    let provider = gtk::CssProvider::new();
    let _ = provider.load_from_data(CSS.as_bytes());
    if let Some(screen) = gtk::gdk::Screen::default() {
        gtk::StyleContext::add_provider_for_screen(
            &screen,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}

pub fn configure(window: &Window, chrome: &Webview, remote: Option<&Webview>, full_window: bool) {
    let chrome_window = window.clone();
    let _ = chrome.with_webview(move |platform| {
        let view = platform.inner();
        let Ok(vbox) = chrome_window.default_vbox() else {
            return;
        };
        if let Some(topbar) = find_by_name(&vbox, TOPBAR) {
            topbar.show_all();
            for name in [BACK, REFRESH] {
                if let Some(widget) = find_by_name(&topbar, name) {
                    widget.set_sensitive(!full_window);
                }
            }
        }
        if full_window {
            view.show_all();
            view.set_vexpand(true);
        } else {
            view.hide();
            view.set_vexpand(false);
        }
    });

    if let Some(remote) = remote {
        let _ = remote.with_webview(move |platform| {
            let view = platform.inner();
            if full_window {
                view.hide();
                view.set_vexpand(false);
            } else {
                view.show_all();
                view.set_vexpand(true);
            }
        });
    }
}

pub fn set_language(app: &AppHandle, language: &str) {
    let english = language.eq_ignore_ascii_case("en");
    let runner = app.clone();
    let app = app.clone();
    let _ = runner.run_on_main_thread(move || {
        let Some(window) = app.get_window(crate::layout::WINDOW_LABEL) else {
            return;
        };
        let Ok(vbox) = window.default_vbox() else {
            return;
        };
        let Some(topbar) = find_by_name(&vbox, TOPBAR) else {
            return;
        };
        let title = if english {
            "DSH Remote Companion"
        } else {
            "DSH 远控伴侣"
        };
        let settings = if english { "Settings" } else { "设置" };
        let back = if english { "Back" } else { "返回" };
        let refresh = if english { "Refresh" } else { "刷新" };
        let back_tooltip = if english {
            "Back to connection page"
        } else {
            "回到连接页"
        };
        let about = if english { "About" } else { "关于" };
        let language_label = if english { "English" } else { "中文" };
        set_label(&topbar, TITLE, title);
        set_label(&topbar, SETTINGS, settings);
        set_label(&topbar, BACK, back);
        set_label(&topbar, REFRESH, refresh);
        set_label(&topbar, ABOUT, about);
        set_label(&topbar, LANGUAGE, language_label);
        set_tooltip(&topbar, SETTINGS, settings);
        set_tooltip(&topbar, BACK, back_tooltip);
        set_tooltip(&topbar, REFRESH, refresh);
        set_tooltip(&topbar, ABOUT, about);
        set_tooltip(&topbar, LANGUAGE, "切换语言 / Switch language");
    });
}

pub fn set_theme(app: &AppHandle, theme: &str) {
    let light = theme == "light";
    let runner = app.clone();
    let app = app.clone();
    let _ = runner.run_on_main_thread(move || {
        let Some(window) = app.get_window(crate::layout::WINDOW_LABEL) else {
            return;
        };
        let Ok(vbox) = window.default_vbox() else {
            return;
        };
        let Some(topbar) = find_by_name(&vbox, TOPBAR) else {
            return;
        };
        if light {
            topbar.style_context().add_class("light");
        } else {
            topbar.style_context().remove_class("light");
        }
    });
}

fn set_label(root: &impl IsA<gtk::Widget>, name: &str, text: &str) {
    if let Some(widget) = find_by_name(root, name) {
        if let Ok(button) = widget.clone().downcast::<gtk::Button>() {
            button.set_label(text);
        } else if let Ok(label) = widget.downcast::<gtk::Label>() {
            label.set_text(text);
        }
    }
}

fn set_tooltip(root: &impl IsA<gtk::Widget>, name: &str, text: &str) {
    if let Some(widget) = find_by_name(root, name) {
        widget.set_tooltip_text(Some(text));
    }
}

fn find_by_name(root: &impl IsA<gtk::Widget>, name: &str) -> Option<gtk::Widget> {
    let root: gtk::Widget = root.clone().upcast();
    if root.widget_name() == name {
        return Some(root);
    }
    if let Ok(container) = root.clone().downcast::<gtk::Container>() {
        for child in container.children() {
            if let Some(found) = find_by_name(&child, name) {
                return Some(found);
            }
        }
    }
    None
}
