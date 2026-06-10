//! Cost / token-spend window, rendering the engine `/cost` payload. Mirrors the
//! macOS cost menu surfaces (today + 30-day totals, a daily history list, and
//! per-model breakdowns). Cost data is local-only and engine-supported for
//! Claude and Codex; other providers report nothing and are shown as such.

use crate::model::{CostDailyEntry, CostPayload};
use crate::providers::branding;
use gtk4::prelude::*;
use gtk4::{Align, Box as GtkBox, Label, Orientation, Separator};
use libadwaita::prelude::*;

/// Open (or re-open) the cost window for the given cost payloads.
pub fn open(app: &libadwaita::Application, costs: &[CostPayload]) {
    let window = libadwaita::ApplicationWindow::builder()
        .application(app)
        .title("CodexBar Cost")
        .default_width(460)
        .default_height(640)
        .build();

    let toolbar = libadwaita::ToolbarView::new();
    toolbar.add_top_bar(&libadwaita::HeaderBar::new());

    let scroller = gtk4::ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .vexpand(true)
        .build();
    scroller.set_child(Some(&build_content(costs)));
    toolbar.set_content(Some(&scroller));
    window.set_content(Some(&toolbar));
    window.present();
}

fn build_content(costs: &[CostPayload]) -> GtkBox {
    let root = GtkBox::new(Orientation::Vertical, 10);
    root.set_margin_top(12);
    root.set_margin_bottom(12);
    root.set_margin_start(12);
    root.set_margin_end(12);

    let reporting: Vec<&CostPayload> = costs
        .iter()
        .filter(|c| c.error.is_none() && (c.session_cost_usd.is_some() || !c.daily.is_empty()))
        .collect();

    if reporting.is_empty() {
        let empty = Label::new(Some(
            "No local cost data.\nCost estimates come from the local token ledger \
and are available for Claude and Codex.",
        ));
        empty.set_justify(gtk4::Justification::Center);
        empty.add_css_class("dim-label");
        root.append(&empty);
        return root;
    }

    for (i, c) in reporting.iter().enumerate() {
        if i > 0 {
            root.append(&Separator::new(Orientation::Horizontal));
        }
        root.append(&build_provider_cost(c));
    }
    root
}

fn build_provider_cost(c: &CostPayload) -> GtkBox {
    let card = GtkBox::new(Orientation::Vertical, 6);
    let currency = c.currency().to_string();

    let title = Label::new(Some(&branding(&c.provider).display_name));
    title.add_css_class("title-4");
    title.set_halign(Align::Start);
    card.append(&title);

    let subtitle = Label::new(Some("API-rate estimate from local token ledger"));
    subtitle.add_css_class("dim-label");
    subtitle.add_css_class("caption");
    subtitle.set_halign(Align::Start);
    card.append(&subtitle);

    // Headline KPIs: today and last-30-days spend + tokens.
    let kpis = GtkBox::new(Orientation::Horizontal, 16);
    kpis.set_margin_top(4);
    kpis.append(&kpi_block(
        "Today",
        &money(c.session_cost_usd, &currency),
        c.session_tokens.map(tokens_short),
    ));
    kpis.append(&kpi_block(
        &format!("Last {} days", c.history_days.unwrap_or(30)),
        &money(c.last_30_days_cost_usd, &currency),
        c.last_30_days_tokens.map(tokens_short),
    ));
    card.append(&kpis);

    // Token totals breakdown (input / output / cache), when reported.
    if let Some(t) = &c.totals {
        if let Some(section) = totals_section(t) {
            card.append(&section);
        }
    }

    // Daily history, newest first, preceded by a spend chart.
    if !c.daily.is_empty() {
        let header = Label::new(Some("Daily history"));
        header.add_css_class("caption-heading");
        header.set_halign(Align::Start);
        header.set_margin_top(6);
        card.append(&header);

        card.append(&spend_chart(&c.daily));

        for entry in c.daily.iter().rev() {
            card.append(&daily_row(entry, &currency));
        }
    }

    card
}

/// Token totals breakdown row: input / output / cache read / cache creation.
/// Returns `None` when no token components are present.
fn totals_section(t: &crate::model::CostTotals) -> Option<GtkBox> {
    let rows: Vec<(&str, Option<i64>)> = vec![
        ("Input", t.total_input_tokens),
        ("Output", t.total_output_tokens),
        ("Cache read", t.cache_read_tokens),
        ("Cache write", t.cache_creation_tokens),
        ("Total", t.total_tokens),
    ];
    if rows.iter().all(|(_, v)| v.is_none()) {
        return None;
    }

    let section = GtkBox::new(Orientation::Vertical, 2);
    section.set_margin_top(6);
    let header = Label::new(Some("Token breakdown"));
    header.add_css_class("caption-heading");
    header.set_halign(Align::Start);
    section.append(&header);

    for (label, value) in rows {
        let Some(v) = value else { continue };
        let row = GtkBox::new(Orientation::Horizontal, 6);
        let l = Label::new(Some(label));
        l.add_css_class("caption");
        l.add_css_class("dim-label");
        l.set_halign(Align::Start);
        row.append(&l);
        let r = Label::new(Some(&format!("{} tokens", tokens_short(v))));
        r.add_css_class("caption");
        r.set_halign(Align::End);
        r.set_hexpand(true);
        row.append(&r);
        section.append(&row);
    }
    Some(section)
}

/// A small cairo bar chart of daily spend (chronological, oldest left). Mirrors
/// the macOS cost history chart at panel scale.
fn spend_chart(daily: &[CostDailyEntry]) -> gtk4::DrawingArea {
    let costs: Vec<f64> = daily.iter().map(|d| d.cost_usd.unwrap_or(0.0)).collect();
    let max = costs.iter().cloned().fold(0.0_f64, f64::max);

    let area = gtk4::DrawingArea::new();
    area.set_content_height(96);
    area.set_hexpand(true);
    area.set_margin_top(4);
    area.set_margin_bottom(4);

    area.set_draw_func(move |_, cr, width, height| {
        let w = width as f64;
        let h = height as f64;
        if costs.is_empty() || max <= 0.0 {
            return;
        }
        let n = costs.len();
        let gap = 3.0;
        let bar_w = ((w - gap * (n as f64 - 1.0)) / n as f64).max(1.0);

        for (i, c) in costs.iter().enumerate() {
            let frac = (c / max).clamp(0.0, 1.0);
            let bar_h = (h - 2.0) * frac;
            let x = i as f64 * (bar_w + gap);
            let y = h - bar_h;
            // Accent blue, brighter for the most recent (last) bar.
            if i + 1 == n {
                cr.set_source_rgba(0.26, 0.52, 0.96, 1.0);
            } else {
                cr.set_source_rgba(0.26, 0.52, 0.96, 0.55);
            }
            cr.rectangle(x, y, bar_w, bar_h);
            let _ = cr.fill();
        }
    });
    area
}

fn kpi_block(label: &str, value: &str, tokens: Option<String>) -> GtkBox {
    let b = GtkBox::new(Orientation::Vertical, 2);
    b.set_hexpand(true);

    let l = Label::new(Some(label));
    l.add_css_class("caption");
    l.add_css_class("dim-label");
    l.set_halign(Align::Start);
    b.append(&l);

    let v = Label::new(Some(value));
    v.add_css_class("title-3");
    v.set_halign(Align::Start);
    b.append(&v);

    if let Some(tok) = tokens {
        let t = Label::new(Some(&format!("{tok} tokens")));
        t.add_css_class("caption");
        t.add_css_class("dim-label");
        t.set_halign(Align::Start);
        b.append(&t);
    }
    b
}

fn daily_row(entry: &CostDailyEntry, currency: &str) -> GtkBox {
    let row = GtkBox::new(Orientation::Vertical, 1);
    row.set_margin_top(2);

    let top = GtkBox::new(Orientation::Horizontal, 6);
    let date = Label::new(Some(&entry.date));
    date.add_css_class("caption");
    date.set_halign(Align::Start);
    top.append(&date);

    let cost = Label::new(Some(&money(entry.cost_usd, currency)));
    cost.add_css_class("caption");
    cost.set_halign(Align::End);
    cost.set_hexpand(true);
    top.append(&cost);
    row.append(&top);

    // Per-day token detail (input / output / cache), when present.
    let mut token_parts: Vec<String> = Vec::new();
    if let Some(v) = entry.input_tokens {
        token_parts.push(format!("in {}", tokens_short(v)));
    }
    if let Some(v) = entry.output_tokens {
        token_parts.push(format!("out {}", tokens_short(v)));
    }
    if let Some(v) = entry.cache_read_tokens {
        token_parts.push(format!("cache-r {}", tokens_short(v)));
    }
    if let Some(v) = entry.cache_creation_tokens {
        token_parts.push(format!("cache-w {}", tokens_short(v)));
    }
    if token_parts.is_empty() {
        if let Some(v) = entry.total_tokens {
            token_parts.push(format!("{} tokens", tokens_short(v)));
        }
    }
    if !token_parts.is_empty() {
        let toks = Label::new(Some(&token_parts.join(" · ")));
        toks.add_css_class("caption");
        toks.add_css_class("dim-label");
        toks.set_halign(Align::Start);
        toks.set_xalign(0.0);
        row.append(&toks);
    }

    // Model breakdown line, if present; else fall back to the model name list.
    if let Some(models) = &entry.model_breakdowns {
        if !models.is_empty() {
            let parts: Vec<String> = models
                .iter()
                .map(|m| {
                    let tok = m
                        .total_tokens
                        .map(|t| format!(" ({})", tokens_short(t)))
                        .unwrap_or_default();
                    format!("{} {}{tok}", m.model_name, money(m.cost_usd, currency))
                })
                .collect();
            let detail = Label::new(Some(&parts.join(" · ")));
            detail.add_css_class("caption");
            detail.add_css_class("dim-label");
            detail.set_halign(Align::Start);
            detail.set_wrap(true);
            detail.set_xalign(0.0);
            row.append(&detail);
        }
    } else if let Some(models) = &entry.models_used {
        if !models.is_empty() {
            let detail = Label::new(Some(&models.join(", ")));
            detail.add_css_class("caption");
            detail.add_css_class("dim-label");
            detail.set_halign(Align::Start);
            detail.set_wrap(true);
            detail.set_xalign(0.0);
            row.append(&detail);
        }
    }
    row
}

fn money(amount: Option<f64>, currency: &str) -> String {
    match amount {
        Some(v) => {
            let symbol = match currency {
                "USD" => "$",
                "EUR" => "€",
                "GBP" => "£",
                _ => "",
            };
            if symbol.is_empty() {
                format!("{v:.2} {currency}")
            } else {
                format!("{symbol}{v:.2}")
            }
        }
        None => "—".to_string(),
    }
}

/// Compact token count, e.g. 39_152_164 -> "39.2M".
fn tokens_short(n: i64) -> String {
    let n = n as f64;
    if n >= 1_000_000_000.0 {
        format!("{:.1}B", n / 1_000_000_000.0)
    } else if n >= 1_000_000.0 {
        format!("{:.1}M", n / 1_000_000.0)
    } else if n >= 1_000.0 {
        format!("{:.1}K", n / 1_000.0)
    } else {
        format!("{n:.0}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn money_formats_usd_symbol() {
        assert_eq!(money(Some(5.312_425), "USD"), "$5.31");
        assert_eq!(money(None, "USD"), "—");
        assert_eq!(money(Some(3.0), "JPY"), "3.00 JPY");
    }

    #[test]
    fn tokens_short_scales() {
        assert_eq!(tokens_short(39_152_164), "39.2M");
        assert_eq!(tokens_short(1_500), "1.5K");
        assert_eq!(tokens_short(289_257_016), "289.3M");
        assert_eq!(tokens_short(2_500_000_000), "2.5B");
    }
}
