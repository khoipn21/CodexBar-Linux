//! libadwaita settings window. Mirrors the macOS Preferences panes that are
//! meaningful on Linux: Providers (per-provider config + login), General
//! (refresh cadence, launch at login), Display, Advanced, About.
//!
//! Provider mutations persist to ~/.codexbar/config.json via ConfigStore, so
//! the CLI and GUI stay in sync.

use crate::config_store::{ConfigStore, ProviderEntry};
use crate::login;
use crate::providers::branding;
use gtk4::prelude::*;
use libadwaita::prelude::*;
use libadwaita::{
    ActionRow, ComboRow, EntryRow, PasswordEntryRow, PreferencesGroup, PreferencesPage,
    PreferencesWindow, SwitchRow,
};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::Mutex;

const SOURCE_MODES: [&str; 5] = ["auto", "web", "cli", "oauth", "api"];
const COOKIE_SOURCES: [&str; 3] = ["auto", "manual", "off"];

/// Open (or re-open) the settings window.
pub fn open(parent: &impl IsA<gtk4::Window>, store: Arc<Mutex<ConfigStore>>) {
    let window = PreferencesWindow::builder()
        .title("CodexBar Settings")
        .modal(false)
        .search_enabled(true)
        .default_width(640)
        .default_height(720)
        .build();
    window.set_transient_for(Some(parent));

    window.add(&build_providers_page(store.clone()));
    window.add(&build_general_page(store.clone()));
    window.add(&build_display_page(store.clone()));
    window.add(&build_advanced_page(store.clone()));
    window.add(&build_about_page());

    window.present();
}

fn build_providers_page(store: Arc<Mutex<ConfigStore>>) -> PreferencesPage {
    let page = PreferencesPage::builder()
        .title("Providers")
        .icon_name("network-server-symbolic")
        .build();

    let providers = store.lock().unwrap().load_providers().unwrap_or_default();
    let group = PreferencesGroup::builder()
        .title("Providers")
        .description("Enable and configure each AI coding provider. Changes are written to ~/.codexbar/config.json.")
        .build();

    for entry in providers {
        group.add(&build_provider_row(entry, store.clone()));
    }
    page.add(&group);
    page
}

/// One expander row per provider: enable switch + a detail area with source,
/// cookie, API key, host/region/workspace, and a login/diagnose button.
fn build_provider_row(entry: ProviderEntry, store: Arc<Mutex<ConfigStore>>) -> libadwaita::ExpanderRow {
    let id = entry.id().to_string();
    let b = branding(&id);
    let entry = Rc::new(std::cell::RefCell::new(entry));

    let row = libadwaita::ExpanderRow::builder()
        .title(&b.display_name)
        .subtitle(&id)
        .build();

    // Enable toggle in the row prefix.
    let enable = gtk4::Switch::builder()
        .active(entry.borrow().enabled())
        .valign(gtk4::Align::Center)
        .build();
    {
        let store = store.clone();
        let id = id.clone();
        enable.connect_state_set(move |_, state| {
            if let Ok(s) = store.lock() {
                let _ = s.set_enabled(&id, state);
            }
            glib::Propagation::Proceed
        });
    }
    row.add_prefix(&enable);

    // Source mode combo.
    let source = ComboRow::builder().title("Source").build();
    let source_model = gtk4::StringList::new(&SOURCE_MODES);
    source.set_model(Some(&source_model));
    let cur_source = entry.borrow().str_field("source").unwrap_or_else(|| "auto".into());
    if let Some(idx) = SOURCE_MODES.iter().position(|m| *m == cur_source) {
        source.set_selected(idx as u32);
    }
    {
        let store = store.clone();
        let entry = entry.clone();
        source.connect_selected_notify(move |c| {
            let v = SOURCE_MODES.get(c.selected() as usize).copied().unwrap_or("auto");
            entry.borrow_mut().set_str("source", Some(v));
            persist(&store, &entry.borrow());
        });
    }
    row.add_row(&source);

    // Cookie source combo.
    let cookie = ComboRow::builder().title("Cookie source").build();
    let cookie_model = gtk4::StringList::new(&COOKIE_SOURCES);
    cookie.set_model(Some(&cookie_model));
    let cur_cookie = entry.borrow().str_field("cookieSource").unwrap_or_else(|| "auto".into());
    if let Some(idx) = COOKIE_SOURCES.iter().position(|m| *m == cur_cookie) {
        cookie.set_selected(idx as u32);
    }
    {
        let store = store.clone();
        let entry = entry.clone();
        cookie.connect_selected_notify(move |c| {
            let v = COOKIE_SOURCES.get(c.selected() as usize).copied().unwrap_or("auto");
            entry.borrow_mut().set_str("cookieSource", Some(v));
            persist(&store, &entry.borrow());
        });
    }
    row.add_row(&cookie);

    // Manual cookie header (secret).
    let cookie_header = PasswordEntryRow::builder().title("Cookie header").build();
    if let Some(h) = entry.borrow().str_field("cookieHeader") {
        cookie_header.set_text(&h);
    }
    {
        let store = store.clone();
        let entry = entry.clone();
        cookie_header.connect_apply(move |e| {
            entry.borrow_mut().set_str("cookieHeader", Some(&e.text()));
            persist(&store, &entry.borrow());
        });
    }
    row.add_row(&cookie_header);

    // Optional host / region / workspace fields.
    for (field, title) in [
        ("enterpriseHost", "Host / base URL"),
        ("region", "Region"),
        ("workspaceID", "Workspace / project ID"),
    ] {
        let e = EntryRow::builder().title(title).build();
        if let Some(v) = entry.borrow().str_field(field) {
            e.set_text(&v);
        }
        let store = store.clone();
        let entry = entry.clone();
        let field = field.to_string();
        e.connect_apply(move |e| {
            entry.borrow_mut().set_str(&field, Some(&e.text()));
            persist(&store, &entry.borrow());
        });
        row.add_row(&e);
    }

    // API key entry (secret) + login/diagnose actions.
    let api_row = ActionRow::builder().title("Authentication").build();
    let key_btn = gtk4::Button::builder()
        .label("Set API key…")
        .valign(gtk4::Align::Center)
        .build();
    {
        let store = store.clone();
        let id = id.clone();
        key_btn.connect_clicked(move |btn| {
            login::api_key::prompt(btn, &id, store.clone());
        });
    }
    let diag_btn = gtk4::Button::builder()
        .label("Diagnose")
        .valign(gtk4::Align::Center)
        .build();
    {
        let store = store.clone();
        let id = id.clone();
        diag_btn.connect_clicked(move |btn| {
            login::cli_check::run(btn, &id, store.clone());
        });
    }
    api_row.add_suffix(&key_btn);
    api_row.add_suffix(&diag_btn);
    row.add_row(&api_row);

    // Automatic browser cookie import for cookie-only (category-2) providers.
    if crate::web::supports_cookie_import(&id) {
        let import_row = ActionRow::builder()
            .title("Browser cookies")
            .subtitle("Import session cookies from your browser (Chrome/Chromium/Brave/Edge/Firefox)")
            .build();
        let import_btn = gtk4::Button::builder()
            .label("Import from browser")
            .valign(gtk4::Align::Center)
            .build();
        let store2 = store.clone();
        let entry2 = entry.clone();
        let id2 = id.clone();
        import_btn.connect_clicked(move |btn| {
            match crate::web::import_cookie_header(&id2) {
                Ok(header) => {
                    {
                        let mut e = entry2.borrow_mut();
                        e.set_str("cookieSource", Some("manual"));
                        e.set_str("cookieHeader", Some(&header));
                    }
                    persist(&store2, &entry2.borrow());
                    show_info(
                        btn,
                        "Cookies imported",
                        "Session cookies were imported and saved. The provider will use them on the next refresh.",
                    );
                }
                Err(e) => show_info(
                    btn,
                    "Cookie import failed",
                    &format!(
                        "{e}\n\nYou can paste a cookie header manually instead: open the provider site \
in your browser, copy the request `Cookie:` header from DevTools, and set Cookie source = manual."
                    ),
                ),
            }
        });
        import_row.add_suffix(&import_btn);
        row.add_row(&import_row);
    }

    row
}

fn persist(store: &Arc<Mutex<ConfigStore>>, entry: &ProviderEntry) {
    if let Ok(s) = store.lock() {
        if let Err(e) = s.save_provider_fields(entry) {
            log::warn!("failed to save provider {}: {e}", entry.id());
        }
    }
}

fn build_general_page(store: Arc<Mutex<ConfigStore>>) -> PreferencesPage {
    let page = PreferencesPage::builder()
        .title("General")
        .icon_name("preferences-system-symbolic")
        .build();
    let group = PreferencesGroup::builder().title("Refresh").build();

    let cadence = ComboRow::builder().title("Refresh frequency").build();
    let model = gtk4::StringList::new(&["Manual", "1 min", "2 min", "5 min", "15 min", "30 min"]);
    cadence.set_model(Some(&model));
    let current = store
        .lock()
        .unwrap()
        .get_app_field("refreshFrequency")
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_else(|| "5 min".into());
    cadence.set_selected(["Manual", "1 min", "2 min", "5 min", "15 min", "30 min"]
        .iter()
        .position(|v| *v == current)
        .unwrap_or(3) as u32);
    {
        let store = store.clone();
        cadence.connect_selected_notify(move |row| {
            let values = ["Manual", "1 min", "2 min", "5 min", "15 min", "30 min"];
            if let Some(value) = values.get(row.selected() as usize) {
                if let Err(e) = store.lock().unwrap().set_app_field("refreshFrequency", serde_json::Value::String((*value).into())) {
                    log::warn!("failed to save refresh frequency: {e}");
                }
            }
        });
    }
    group.add(&cadence);

    let launch = SwitchRow::builder()
        .title("Launch at login")
        .subtitle("Start CodexBar automatically after you sign in")
        .active(crate::autostart::is_enabled())
        .build();
    launch.connect_active_notify(|row| {
        if let Err(e) = crate::autostart::set_enabled(row.is_active()) {
            log::warn!("failed to update autostart: {e}");
        }
    });
    group.add(&launch);

    let notify = SwitchRow::builder()
        .title("Session quota notifications")
        .active(store.lock().unwrap().get_app_field("quotaNotifications").and_then(|v| v.as_bool()).unwrap_or(true))
        .build();
    {
        let store = store.clone();
        notify.connect_active_notify(move |row| {
            if let Err(e) = store.lock().unwrap().set_app_field("quotaNotifications", serde_json::Value::Bool(row.is_active())) {
                log::warn!("failed to save notification setting: {e}");
            }
        });
    }
    group.add(&notify);

    page.add(&group);
    page
}

fn build_display_page(store: Arc<Mutex<ConfigStore>>) -> PreferencesPage {
    let page = PreferencesPage::builder()
        .title("Display")
        .icon_name("preferences-desktop-display-symbolic")
        .build();
    let group = PreferencesGroup::builder()
        .title("Panel display")
        .description("Linux equivalents of CodexBar menu-bar display controls")
        .build();

    let mode = ComboRow::builder().title("Tray metric").build();
    let modes = ["Lowest remaining", "Highest usage", "Codex session", "Codex weekly", "Credits"];
    mode.set_model(Some(&gtk4::StringList::new(&modes)));
    let current = store.lock().unwrap().get_app_field("trayMetric").and_then(|v| v.as_str().map(str::to_string)).unwrap_or_else(|| modes[0].into());
    mode.set_selected(modes.iter().position(|v| *v == current).unwrap_or(0) as u32);
    {
        let store = store.clone();
        mode.connect_selected_notify(move |row| {
            let modes = ["Lowest remaining", "Highest usage", "Codex session", "Codex weekly", "Credits"];
            if let Some(value) = modes.get(row.selected() as usize) {
                if let Err(e) = store.lock().unwrap().set_app_field("trayMetric", serde_json::Value::String((*value).into())) {
                    log::warn!("failed to save tray metric: {e}");
                }
            }
        });
    }
    group.add(&mode);

    let show_text = SwitchRow::builder()
        .title("Show text in panel utility")
        .subtitle("Keep provider names and exact percentages visible instead of icon-only status")
        .active(store.lock().unwrap().get_app_field("showPanelText").and_then(|v| v.as_bool()).unwrap_or(true))
        .build();
    {
        let store = store.clone();
        show_text.connect_active_notify(move |row| {
            if let Err(e) = store.lock().unwrap().set_app_field("showPanelText", serde_json::Value::Bool(row.is_active())) {
                log::warn!("failed to save display text setting: {e}");
            }
        });
    }
    group.add(&show_text);

    page.add(&group);
    page
}

fn build_advanced_page(store: Arc<Mutex<ConfigStore>>) -> PreferencesPage {
    let page = PreferencesPage::builder()
        .title("Advanced")
        .icon_name("applications-engineering-symbolic")
        .build();
    let group = PreferencesGroup::builder().title("Advanced").build();

    let status = SwitchRow::builder()
        .title("Check provider status")
        .subtitle("Poll provider status pages and show incident badges")
        .build();
    group.add(&status);

    let validate_row = ActionRow::builder()
        .title("Validate config")
        .subtitle("Run codexbar config validate")
        .build();
    let validate_btn = gtk4::Button::builder()
        .label("Validate")
        .valign(gtk4::Align::Center)
        .build();
    {
        let store = store.clone();
        validate_btn.connect_clicked(move |btn| {
            let result = store.lock().unwrap().validate();
            let (heading, body) = match result {
                Ok(s) if s.trim() == "[]" => ("Config is valid".to_string(), String::new()),
                Ok(s) => ("Validation warnings".to_string(), s),
                Err(e) => ("Validation error".to_string(), e.to_string()),
            };
            show_info(btn, &heading, &body);
        });
    }
    validate_row.add_suffix(&validate_btn);
    group.add(&validate_row);

    page.add(&group);
    page
}

fn build_about_page() -> PreferencesPage {
    let page = PreferencesPage::builder()
        .title("About")
        .icon_name("help-about-symbolic")
        .build();
    let group = PreferencesGroup::builder().title("CodexBar for Linux").build();
    let row = ActionRow::builder()
        .title("CodexBar")
        .subtitle("Every AI coding limit in your panel. Engine: steipete/CodexBar (Swift). GUI: GTK4/libadwaita.")
        .build();
    group.add(&row);
    page.add(&group);
    page
}

/// Minimal info dialog used by validate/login flows.
pub fn show_info(anchor: &impl IsA<gtk4::Widget>, heading: &str, body: &str) {
    let dialog = libadwaita::MessageDialog::builder()
        .heading(heading)
        .body(body)
        .build();
    if let Some(root) = anchor.root().and_downcast::<gtk4::Window>() {
        dialog.set_transient_for(Some(&root));
    }
    dialog.add_response("ok", "OK");
    dialog.present();
}
