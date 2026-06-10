//! Compact panel utility window for desktop panels that do not expose the full
//! StatusNotifierItem menu surface consistently.

use crate::model::ProviderPayload;
use crate::popover;
use gtk4::prelude::*;
use gtk4::{Align, Box as GtkBox, Button, Label, Orientation};
use libadwaita::prelude::*;

pub fn open(
    app: &libadwaita::Application,
    payloads: &[ProviderPayload],
    on_refresh: impl Fn() + Clone + 'static,
    on_settings: impl Fn() + Clone + 'static,
) {
    let window = libadwaita::ApplicationWindow::builder()
        .application(app)
        .title("CodexBar Panel")
        .default_width(420)
        .default_height(620)
        .build();

    let toolbar = libadwaita::ToolbarView::new();
    let header = libadwaita::HeaderBar::new();

    let refresh = Button::builder()
        .icon_name("view-refresh-symbolic")
        .tooltip_text("Refresh usage")
        .valign(Align::Center)
        .build();
    refresh.connect_clicked(move |_| on_refresh());
    header.pack_start(&refresh);

    let settings = Button::builder()
        .icon_name("emblem-system-symbolic")
        .tooltip_text("Open settings")
        .valign(Align::Center)
        .build();
    settings.connect_clicked(move |_| on_settings());
    header.pack_end(&settings);

    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&build_panel_content(payloads)));
    window.set_content(Some(&toolbar));
    window.present();
}

fn build_panel_content(payloads: &[ProviderPayload]) -> GtkBox {
    let root = GtkBox::new(Orientation::Vertical, 12);
    root.set_margin_top(12);
    root.set_margin_bottom(12);
    root.set_margin_start(12);
    root.set_margin_end(12);

    let title = Label::new(Some("Panel utility"));
    title.add_css_class("title-2");
    title.set_halign(Align::Start);
    root.append(&title);

    let summary = Label::new(Some(&summary_text(payloads)));
    summary.add_css_class("dim-label");
    summary.set_halign(Align::Start);
    summary.set_wrap(true);
    root.append(&summary);

    let scroller = gtk4::ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .vexpand(true)
        .build();
    scroller.set_child(Some(&popover::build_provider_list(payloads)));
    root.append(&scroller);

    root
}

fn summary_text(payloads: &[ProviderPayload]) -> String {
    let enabled = payloads.len();
    let errors = payloads.iter().filter(|p| p.error.is_some()).count();
    let incidents = payloads
        .iter()
        .filter(|p| p.status.as_ref().map(|s| s.is_incident()).unwrap_or(false))
        .count();
    let lowest = payloads
        .iter()
        .filter_map(|p| p.headline_window().map(|w| (p.provider.as_str(), w.remaining_percent())))
        .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

    match lowest {
        Some((provider, remaining)) => format!(
            "{enabled} providers · {provider} lowest at {}% remaining · {errors} errors · {incidents} incidents",
            remaining.round() as i64
        ),
        None => format!("{enabled} providers · {errors} errors · {incidents} incidents"),
    }
}
