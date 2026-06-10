//! API-key entry flow: prompt for a secret and store it via
//! `codexbar config set-api-key --stdin` (engine trims + sets 0600 + enables).

use crate::config_store::ConfigStore;
use gtk4::prelude::*;
use libadwaita::prelude::*;
use std::sync::{Arc, Mutex};

pub fn prompt(anchor: &impl IsA<gtk4::Widget>, provider_id: &str, store: Arc<Mutex<ConfigStore>>) {
    let dialog = libadwaita::MessageDialog::builder()
        .heading(format!("Set API key for {provider_id}"))
        .body("The key is stored in ~/.codexbar/config.json with 0600 permissions and never logged.")
        .build();
    if let Some(root) = anchor.root().and_downcast::<gtk4::Window>() {
        dialog.set_transient_for(Some(&root));
    }

    let entry = libadwaita::PasswordEntryRow::builder().title("API key").build();
    let group = libadwaita::PreferencesGroup::new();
    group.add(&entry);
    dialog.set_extra_child(Some(&group));

    dialog.add_response("cancel", "Cancel");
    dialog.add_response("save", "Save");
    dialog.set_response_appearance("save", libadwaita::ResponseAppearance::Suggested);
    dialog.set_default_response(Some("save"));

    let provider_id = provider_id.to_string();
    dialog.connect_response(None, move |dlg, resp| {
        if resp == "save" {
            let key = entry.text().to_string();
            if !key.is_empty() {
                let res = store.lock().unwrap().set_api_key(&provider_id, &key, true);
                if let Err(e) = res {
                    log::warn!("set-api-key failed for {provider_id}: {e}");
                }
            }
        }
        dlg.close();
    });
    dialog.present();
}
