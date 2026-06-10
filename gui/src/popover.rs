//! The usage popover window: a libadwaita window listing provider cards built
//! from the latest engine snapshot. Built imperatively so it can be rebuilt on
//! each refresh.

use crate::format::{remaining_label, reset_label};
use crate::model::ProviderPayload;
use crate::providers::branding;
use gtk4::prelude::*;
use gtk4::{Align, Box as GtkBox, Label, LevelBar, Orientation, Separator};

/// Build the scrollable provider list widget from the current payloads.
pub fn build_provider_list(payloads: &[ProviderPayload]) -> GtkBox {
    let root = GtkBox::new(Orientation::Vertical, 6);
    root.set_margin_top(8);
    root.set_margin_bottom(8);
    root.set_margin_start(8);
    root.set_margin_end(8);

    if payloads.is_empty() {
        let empty = Label::new(Some("No providers enabled.\nOpen Settings to add one."));
        empty.set_justify(gtk4::Justification::Center);
        empty.add_css_class("dim-label");
        root.append(&empty);
        return root;
    }

    for (i, p) in payloads.iter().enumerate() {
        if i > 0 {
            root.append(&Separator::new(Orientation::Horizontal));
        }
        root.append(&build_card(p));
    }
    root
}

fn build_card(p: &ProviderPayload) -> GtkBox {
    let b = branding(&p.provider);
    let card = GtkBox::new(Orientation::Vertical, 4);

    // Header: provider name + optional account.
    let header = GtkBox::new(Orientation::Horizontal, 6);
    let dot = color_dot(&b.color);
    header.append(&dot);
    let title = Label::new(Some(&b.display_name));
    title.add_css_class("heading");
    title.set_halign(Align::Start);
    header.append(&title);
    if let Some(acct) = p.account.as_ref().or_else(|| {
        p.usage
            .as_ref()
            .and_then(|u| u.identity.as_ref())
            .and_then(|id| id.account_email.as_ref())
    }) {
        let a = Label::new(Some(acct));
        a.add_css_class("dim-label");
        a.add_css_class("caption");
        a.set_halign(Align::End);
        a.set_hexpand(true);
        header.append(&a);
    }
    card.append(&header);

    // Error state: show the engine message (e.g. macOS-web ceiling).
    if let Some(err) = &p.error {
        let msg = Label::new(Some(&err.message));
        msg.add_css_class("dim-label");
        msg.add_css_class("caption");
        msg.set_halign(Align::Start);
        msg.set_wrap(true);
        msg.set_xalign(0.0);
        card.append(&msg);
        return card;
    }

    // Usage windows.
    if let Some(u) = &p.usage {
        append_window(&card, "Session", u.primary.as_ref());
        append_window(&card, "Weekly", u.secondary.as_ref());
        append_window(&card, "Extra", u.tertiary.as_ref());
        if let Some(extras) = &u.extra_rate_windows {
            for nw in extras {
                let name = nw.name.clone().unwrap_or_else(|| "Window".into());
                append_window(&card, &name, Some(&nw.window));
            }
        }
    }

    // Credits.
    if let Some(c) = &p.credits {
        if c.remaining != 0.0 {
            let credits = Label::new(Some(&format!("Credits: {:.2} left", c.remaining)));
            credits.add_css_class("caption");
            credits.set_halign(Align::Start);
            card.append(&credits);
        }
    }

    // Status incident.
    if let Some(s) = &p.status {
        if s.is_incident() {
            let label = s
                .description
                .clone()
                .unwrap_or_else(|| s.indicator.clone());
            let inc = Label::new(Some(&format!("⚠ {label}")));
            inc.add_css_class("caption");
            inc.set_halign(Align::Start);
            card.append(&inc);
        }
    }

    card
}

fn append_window(card: &GtkBox, name: &str, window: Option<&crate::model::RateWindow>) {
    let Some(w) = window else { return };
    let row = GtkBox::new(Orientation::Vertical, 2);

    let top = GtkBox::new(Orientation::Horizontal, 6);
    let label = Label::new(Some(name));
    label.set_halign(Align::Start);
    label.add_css_class("caption-heading");
    top.append(&label);

    let pct = Label::new(Some(&remaining_label(w.remaining_percent())));
    pct.add_css_class("caption");
    pct.set_halign(Align::End);
    pct.set_hexpand(true);
    top.append(&pct);
    row.append(&top);

    // Progress bar shows remaining fraction.
    let bar = LevelBar::new();
    bar.set_min_value(0.0);
    bar.set_max_value(100.0);
    bar.set_value(w.remaining_percent());
    bar.set_hexpand(true);
    row.append(&bar);

    if let Some(reset) = reset_label(w.resets_at.as_deref(), w.reset_description.as_deref()) {
        let r = Label::new(Some(&reset));
        r.add_css_class("dim-label");
        r.add_css_class("caption");
        r.set_halign(Align::Start);
        row.append(&r);
    }

    card.append(&row);
}

fn color_dot(c: &crate::providers::Rgb) -> gtk4::DrawingArea {
    let area = gtk4::DrawingArea::new();
    area.set_content_width(12);
    area.set_content_height(12);
    area.set_valign(Align::Center);
    let (r, g, b) = (c.r, c.g, c.b);
    area.set_draw_func(move |_, cr, w, h| {
        cr.set_source_rgb(r, g, b);
        cr.arc(w as f64 / 2.0, h as f64 / 2.0, w as f64 / 2.0 - 1.0, 0.0, std::f64::consts::TAU);
        let _ = cr.fill();
    });
    area
}
