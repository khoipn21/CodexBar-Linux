//! The usage popover window: a libadwaita window listing provider cards built
//! from the latest engine snapshot. Built imperatively so it can be rebuilt on
//! each refresh.

use crate::format::{pace_label, remaining_label, reset_label};
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

    // Header: provider name + optional account + copy affordance.
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
    } else {
        // Keep the copy button pinned to the trailing edge.
        let spacer = GtkBox::new(Orientation::Horizontal, 0);
        spacer.set_hexpand(true);
        header.append(&spacer);
    }

    // Click-to-copy: copies a one-line summary of this provider to the clipboard.
    let copy = gtk4::Button::builder()
        .icon_name("edit-copy-symbolic")
        .tooltip_text("Copy usage summary")
        .valign(Align::Center)
        .has_frame(false)
        .build();
    let summary = summary_line(&b.display_name, p);
    copy.connect_clicked(move |btn| {
        btn.clipboard().set_text(&summary);
    });
    header.append(&copy);
    card.append(&header);

    // Identity line: source, login method / plan, organization, and engine
    // version, when present.
    {
        let mut parts: Vec<String> = Vec::new();
        if let Some(src) = p.source.as_deref() {
            if !src.is_empty() {
                parts.push(src.to_string());
            }
        }
        if let Some(id) = p.usage.as_ref().and_then(|u| u.identity.as_ref()) {
            if let Some(method) = id.login_method.as_deref() {
                if !method.is_empty() {
                    parts.push(method.to_string());
                }
            }
            if let Some(org) = id.account_organization.as_deref() {
                if !org.is_empty() {
                    parts.push(org.to_string());
                }
            }
        }
        if let Some(version) = p.version.as_deref() {
            if !version.is_empty() {
                parts.push(format!("v{}", version.trim_start_matches('v')));
            }
        }
        if !parts.is_empty() {
            let line = Label::new(Some(&parts.join(" · ")));
            line.add_css_class("caption");
            line.add_css_class("dim-label");
            line.set_halign(Align::Start);
            line.set_xalign(0.0);
            card.append(&line);
        }
    }

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

    // Credits + recent credit events.
    if let Some(c) = &p.credits {
        if c.remaining != 0.0 {
            let credits = Label::new(Some(&format!("Credits: {:.2} left", c.remaining)));
            credits.add_css_class("caption");
            credits.set_halign(Align::Start);
            card.append(&credits);
        }
        for event in c.events.iter().take(3) {
            if let Some(line) = credit_event_line(event) {
                let ev = Label::new(Some(&line));
                ev.add_css_class("caption");
                ev.add_css_class("dim-label");
                ev.set_halign(Align::Start);
                ev.set_wrap(true);
                ev.set_xalign(0.0);
                card.append(&ev);
            }
        }
    }

    // Provider status badge: operational / maintenance / incident, with the
    // last-updated time and a link to the provider status page when present.
    if let Some(s) = &p.status {
        if s.indicator != "none" || s.url.is_some() {
            append_status(&card, s);
        }
    }

    // Last-updated footer from the usage snapshot.
    if let Some(updated) = p
        .usage
        .as_ref()
        .and_then(|u| u.updated_at.as_deref())
        .and_then(updated_label)
    {
        let footer = Label::new(Some(&updated));
        footer.add_css_class("caption");
        footer.add_css_class("dim-label");
        footer.set_halign(Align::Start);
        footer.set_xalign(0.0);
        card.append(&footer);
    }

    card
}

/// "updated 2m ago" style relative footer from an ISO timestamp.
fn updated_label(iso: &str) -> Option<String> {
    let dt = chrono::DateTime::parse_from_rfc3339(iso).ok()?;
    let secs = (chrono::Utc::now() - dt.with_timezone(&chrono::Utc)).num_seconds();
    if secs < 0 {
        return Some("updated just now".to_string());
    }
    let rel = if secs < 60 {
        "just now".to_string()
    } else if secs < 3600 {
        format!("{}m ago", secs / 60)
    } else if secs < 86_400 {
        format!("{}h ago", secs / 3600)
    } else {
        format!("{}d ago", secs / 86_400)
    };
    Some(format!("updated {rel}"))
}

fn append_status(card: &GtkBox, s: &crate::model::ProviderStatus) {
    let (glyph, text) = match s.indicator.as_str() {
        "none" => ("●", "Operational".to_string()),
        "maintenance" => ("◐", "Under maintenance".to_string()),
        "minor" | "major" | "critical" => (
            "⚠",
            s.description.clone().unwrap_or_else(|| format!("{} incident", s.indicator)),
        ),
        other => ("•", s.description.clone().unwrap_or_else(|| other.to_string())),
    };

    let row = GtkBox::new(Orientation::Horizontal, 6);
    let badge = Label::new(Some(&format!("{glyph} {text}")));
    badge.add_css_class("caption");
    badge.set_halign(Align::Start);
    row.append(&badge);

    if let Some(updated) = status_updated_label(s.updated_at.as_deref()) {
        let when = Label::new(Some(&updated));
        when.add_css_class("dim-label");
        when.add_css_class("caption");
        when.set_halign(Align::End);
        when.set_hexpand(true);
        row.append(&when);
    }
    card.append(&row);

    if let Some(url) = &s.url {
        let link = gtk4::LinkButton::builder()
            .uri(url)
            .label("Open status page")
            .halign(Align::Start)
            .has_frame(false)
            .build();
        link.add_css_class("caption");
        card.append(&link);
    }
}

fn status_updated_label(updated_at: Option<&str>) -> Option<String> {
    let iso = updated_at?;
    let dt = chrono::DateTime::parse_from_rfc3339(iso).ok()?;
    Some(format!("updated {}", dt.format("%b %d %H:%M")))
}

fn credit_event_line(e: &crate::model::CreditEvent) -> Option<String> {
    // Render only events that carry meaningful detail.
    let used = e.credits_used?;
    let date = e.date.as_deref().unwrap_or("");
    let service = e.service.as_deref().unwrap_or("usage");
    let date_short = date.split('T').next().unwrap_or(date);
    if date_short.is_empty() {
        Some(format!("• {service}: {used:.2}"))
    } else {
        Some(format!("• {date_short} {service}: {used:.2}"))
    }
}

fn summary_line(name: &str, p: &ProviderPayload) -> String {
    if let Some(err) = &p.error {
        return format!("{name}: {}", err.message);
    }
    let mut parts: Vec<String> = Vec::new();
    if let Some(u) = &p.usage {
        for (label, win) in [
            ("session", u.primary.as_ref()),
            ("weekly", u.secondary.as_ref()),
            ("extra", u.tertiary.as_ref()),
        ] {
            if let Some(w) = win {
                parts.push(format!("{label} {}% left", w.remaining_percent().round() as i64));
            }
        }
    }
    if let Some(c) = &p.credits {
        if c.remaining != 0.0 {
            parts.push(format!("{:.2} credits", c.remaining));
        }
    }
    if parts.is_empty() {
        format!("{name}: no usage data")
    } else {
        format!("{name}: {}", parts.join(", "))
    }
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

    if let Some(pace) = pace_label(w.used_percent, w.window_minutes, w.resets_at.as_deref()) {
        let p = Label::new(Some(&pace));
        p.add_css_class("dim-label");
        p.add_css_class("caption");
        p.set_halign(Align::Start);
        row.append(&p);
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
