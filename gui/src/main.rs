//! CodexBar Linux tray app entry point.
//!
//! Architecture: a ksni StatusNotifierItem tray runs on a background thread and
//! sends `TrayCommand`s into an async-channel. The GTK main loop drains that
//! channel via `spawn_future_local`, owns an `EngineClient` (which spawns
//! `codexbar serve`), a refresh timer, and a libadwaita window that shows
//! provider cards. See docs/system-architecture.md.

mod config_store;
mod engine_client;
mod format;
mod icon_renderer;
mod login;
mod model;
mod popover;
mod providers;
mod settings;
mod tray;
mod web;

use config_store::ConfigStore;
use engine_client::EngineClient;
use gtk4::glib;
use gtk4::prelude::*;
use libadwaita::prelude::*;
use icon_renderer::{render, IconOptions, IconPixmap};
use model::{ProviderPayload, RateWindow};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use tray::{CodexBarTray, TrayCommand};

const APP_ID: &str = "app.codexbar.tray";
const REFRESH_SECS: u64 = 300; // default 5m, matches macOS default

type TrayHandle = ksni::Handle<CodexBarTray>;

fn main() -> glib::ExitCode {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let engine_bin = match EngineClient::locate_binary() {
        Ok(b) => b,
        Err(e) => {
            eprintln!("could not locate codexbar engine: {e}");
            std::process::exit(2);
        }
    };
    let engine = match EngineClient::spawn(&engine_bin, REFRESH_SECS) {
        Ok(c) => Arc::new(Mutex::new(c)),
        Err(e) => {
            eprintln!("failed to start engine ({}): {e}", engine_bin.display());
            std::process::exit(2);
        }
    };

    let app = libadwaita::Application::builder().application_id(APP_ID).build();
    let (tx, rx) = async_channel::unbounded::<TrayCommand>();
    let config = Arc::new(Mutex::new(ConfigStore::new(engine_bin.clone())));

    app.connect_activate(move |app| {
        build_ui(app, engine.clone(), config.clone(), tx.clone(), rx.clone());
    });

    let empty: [&str; 0] = [];
    app.run_with_args(&empty)
}

fn build_ui(
    app: &libadwaita::Application,
    engine: Arc<Mutex<EngineClient>>,
    config: Arc<Mutex<ConfigStore>>,
    tx: async_channel::Sender<TrayCommand>,
    rx: async_channel::Receiver<TrayCommand>,
) {
    let window = libadwaita::ApplicationWindow::builder()
        .application(app)
        .title("CodexBar")
        .default_width(360)
        .default_height(520)
        .build();
    // Closing the window hides it (tray app keeps running).
    window.connect_close_request(|w| {
        w.set_visible(false);
        glib::Propagation::Stop
    });

    let toolbar = libadwaita::ToolbarView::new();
    toolbar.add_top_bar(&libadwaita::HeaderBar::new());
    let scroller = gtk4::ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .vexpand(true)
        .build();
    toolbar.set_content(Some(&scroller));
    window.set_content(Some(&toolbar));

    let payloads: Rc<RefCell<Vec<ProviderPayload>>> = Rc::new(RefCell::new(Vec::new()));

    // Tray service on its own thread.
    let tray_service = ksni::TrayService::new(CodexBarTray::new(
        render(None, &IconOptions::default()),
        "Loading…".into(),
        tx.clone(),
    ));
    let tray_handle: TrayHandle = tray_service.handle();
    tray_service.spawn();

    let refresh: Rc<dyn Fn()> = {
        let engine = engine.clone();
        let payloads = payloads.clone();
        let scroller = scroller.clone();
        let tray_handle = tray_handle.clone();
        Rc::new(move || {
            let result = engine.lock().unwrap().usage(Some("all"));
            match result {
                Ok(list) => {
                    *payloads.borrow_mut() = list.clone();
                    scroller.set_child(Some(&popover::build_provider_list(&list)));
                    update_tray(&tray_handle, &list);
                }
                Err(e) => {
                    log::warn!("refresh failed: {e}");
                    let icon = render(None, &IconOptions { dimmed: true, ..Default::default() });
                    tray_handle.update(move |t: &mut CodexBarTray| {
                        t.set_icon(clone_icon(&icon));
                        t.set_tooltip("Refresh failed".into());
                    });
                }
            }
        })
    };

    refresh();
    {
        let refresh = refresh.clone();
        glib::timeout_add_seconds_local(REFRESH_SECS as u32, move || {
            refresh();
            glib::ControlFlow::Continue
        });
    }

    // Drain tray commands on the GTK main loop.
    {
        let window = window.clone();
        let app = app.clone();
        let refresh = refresh.clone();
        let config = config.clone();
        glib::spawn_future_local(async move {
            while let Ok(cmd) = rx.recv().await {
                match cmd {
                    TrayCommand::ToggleWindow => {
                        if window.is_visible() {
                            window.set_visible(false);
                        } else {
                            window.present();
                        }
                    }
                    TrayCommand::RefreshNow => refresh(),
                    TrayCommand::OpenSettings => settings::open(&window, config.clone()),
                    TrayCommand::Quit => app.quit(),
                }
            }
        });
    }
}

fn clone_icon(icon: &IconPixmap) -> IconPixmap {
    IconPixmap { width: icon.width, height: icon.height, data: icon.data.clone() }
}

fn clone_window(w: &RateWindow) -> RateWindow {
    RateWindow {
        used_percent: w.used_percent,
        window_minutes: w.window_minutes,
        resets_at: w.resets_at.clone(),
        reset_description: w.reset_description.clone(),
        next_regen_percent: w.next_regen_percent,
    }
}

/// Pick the most-urgent provider (lowest remaining%) and update the tray icon.
fn update_tray(handle: &TrayHandle, list: &[ProviderPayload]) {
    let mut headline: Option<(f64, providers::Rgb, RateWindow, bool)> = None;
    for p in list {
        let incident = p.status.as_ref().map(|s| s.is_incident()).unwrap_or(false);
        if let Some(w) = p.headline_window() {
            let remaining = w.remaining_percent();
            let color = providers::branding(&p.provider).color;
            let better = headline.as_ref().map(|(r, ..)| remaining < *r).unwrap_or(true);
            if better {
                headline = Some((remaining, color, clone_window(w), incident));
            }
        }
    }

    let (tooltip, opts, window) = match headline {
        Some((remaining, color, window, incident)) => (
            format!("{}% remaining", remaining.round() as i64),
            IconOptions { accent: color, incident, ..Default::default() },
            Some(window),
        ),
        None => (
            "No usage data".into(),
            IconOptions { dimmed: true, ..Default::default() },
            None,
        ),
    };

    let icon = render(window.as_ref(), &opts);
    handle.update(move |t: &mut CodexBarTray| {
        t.set_icon(clone_icon(&icon));
        t.set_tooltip(tooltip.clone());
    });
}
