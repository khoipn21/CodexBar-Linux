//! Provider login / authentication flows. Each submodule implements one auth
//! kind, mirroring the macOS *LoginFlow files but Linux-native.

pub mod api_key;
pub mod cli_check;
pub mod oauth;
