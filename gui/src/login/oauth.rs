//! OAuth / device-flow login: open the provider's auth URL in the system
//! browser via xdg-open. The engine's file-based OAuth credential stores pick
//! up the resulting tokens (Codex/Claude OAuth, Copilot device flow, Factory
//! WorkOS) on the next fetch. For providers without a headless-drivable flow,
//! users fall back to manual token / API-key entry.

use gtk4::prelude::*;
use crate::settings::show_info;

pub fn open_url(anchor: &impl IsA<gtk4::Widget>, url: &str) {
    match std::process::Command::new("xdg-open").arg(url).spawn() {
        Ok(_) => {
            show_info(
                anchor,
                "Continue in your browser",
                &format!(
                    "Opened {url}\n\nComplete sign-in there. CodexBar will pick up the \
credentials on the next refresh."
                ),
            );
        }
        Err(e) => {
            show_info(
                anchor,
                "Could not open browser",
                &format!("xdg-open failed: {e}\n\nOpen this URL manually:\n{url}"),
            );
        }
    }
}
