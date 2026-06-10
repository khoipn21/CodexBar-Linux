use crate::model::ProviderPayload;
use crate::providers;
use gtk4::gio;
use std::collections::HashSet;

const LOW_REMAINING_THRESHOLD: f64 = 10.0;

pub fn publish(
    app: &impl gio::prelude::ApplicationExt,
    payloads: &[ProviderPayload],
    sent: &mut HashSet<String>,
) {
    for payload in payloads {
        let name = providers::branding(&payload.provider).display_name;
        if let Some(status) = &payload.status {
            if status.is_incident() {
                let key = format!("incident:{}:{}", payload.provider, status.indicator);
                if sent.insert(key) {
                    let body = status
                        .description
                        .clone()
                        .unwrap_or_else(|| status.indicator.clone());
                    send(app, &format!("{name} incident"), &body);
                }
            }
        }

        if let Some(window) = payload.headline_window() {
            let remaining = window.remaining_percent();
            if remaining <= LOW_REMAINING_THRESHOLD {
                let key = format!("quota:{}:{}", payload.provider, window.window_minutes.unwrap_or_default());
                if sent.insert(key) {
                    send(
                        app,
                        &format!("{name} quota low"),
                        &format!("{}% remaining", remaining.round() as i64),
                    );
                }
            }
        }
    }
}

fn send(app: &impl gio::prelude::ApplicationExt, title: &str, body: &str) {
    let notification = gio::Notification::new(title);
    notification.set_body(Some(body));
    app.send_notification(None, &notification);
}
