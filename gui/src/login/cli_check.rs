//! CLI/local provider diagnose flow: run `codexbar usage --provider <id>
//! --source cli --verbose` and show the result, so users can confirm a
//! CLI-backed provider (Codex CLI, Claude PTY, Kiro, Augment, gcloud, …) is
//! reachable using their existing login.

use crate::config_store::ConfigStore;
use crate::settings::show_info;
use gtk4::prelude::*;
use std::sync::{Arc, Mutex};

pub fn run(anchor: &impl IsA<gtk4::Widget>, provider_id: &str, store: Arc<Mutex<ConfigStore>>) {
    let engine = store.lock().unwrap().engine_path();
    let out = std::process::Command::new(&engine)
        .args([
            "usage",
            "--provider",
            provider_id,
            "--source",
            "cli",
            "--format",
            "json",
        ])
        .output();

    let body = match out {
        Ok(o) => {
            let stdout = String::from_utf8_lossy(&o.stdout);
            let stderr = String::from_utf8_lossy(&o.stderr);
            if stdout.trim().is_empty() {
                format!("(no output)\n{stderr}")
            } else {
                stdout.to_string()
            }
        }
        Err(e) => format!("Failed to run engine: {e}"),
    };
    show_info(anchor, &format!("Diagnose: {provider_id}"), &body);
}
