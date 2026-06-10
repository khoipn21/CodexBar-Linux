//! Read/write `~/.codexbar/config.json`, shared with the Swift engine.
//!
//! Strategy: the engine owns normalization, permissions (0600), and secret
//! handling, so for mutations that the CLI exposes (enable/disable, set-api-key)
//! we shell out to `codexbar config`. For the richer per-provider fields the CLI
//! has no setter for (source, cookieSource, cookieHeader, region, workspaceID,
//! enterpriseHost, ordering), we edit the JSON directly with a permissive model
//! that preserves unknown keys so an upstream schema bump is not lost.

use anyhow::{Context, Result};
use serde_json::{Map, Value};
use std::path::PathBuf;
use std::process::Command;

pub struct ConfigStore {
    engine: PathBuf,
    path: PathBuf,
}

/// A single provider entry, kept as a JSON object so unknown fields round-trip.
#[derive(Debug, Clone)]
pub struct ProviderEntry(pub Map<String, Value>);

impl ProviderEntry {
    pub fn id(&self) -> &str {
        self.0.get("id").and_then(Value::as_str).unwrap_or("")
    }
    pub fn enabled(&self) -> bool {
        self.0.get("enabled").and_then(Value::as_bool).unwrap_or(false)
    }
    pub fn str_field(&self, key: &str) -> Option<String> {
        self.0.get(key).and_then(Value::as_str).map(str::to_string)
    }
    pub fn set_str(&mut self, key: &str, value: Option<&str>) {
        match value {
            Some(v) if !v.is_empty() => {
                self.0.insert(key.into(), Value::String(v.into()));
            }
            _ => {
                self.0.insert(key.into(), Value::Null);
            }
        }
    }
}

impl ConfigStore {
    pub fn new(engine: PathBuf) -> Self {
        let path = config_path();
        ConfigStore { engine, path }
    }

    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    pub fn engine_path(&self) -> PathBuf {
        self.engine.clone()
    }

    /// Read the normalized provider list via `codexbar config dump` (so we see
    /// engine defaults), falling back to the on-disk file.
    pub fn load_providers(&self) -> Result<Vec<ProviderEntry>> {
        let dumped = self.run_config(&["dump"]).ok();
        let root: Value = match dumped {
            Some(s) => serde_json::from_str(&s).context("parsing config dump")?,
            None => {
                let text = std::fs::read_to_string(&self.path).unwrap_or_else(|_| {
                    r#"{"version":1,"providers":[]}"#.to_string()
                });
                serde_json::from_str(&text).context("parsing config file")?
            }
        };
        let providers = root
            .get("providers")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        Ok(providers
            .into_iter()
            .filter_map(|v| v.as_object().cloned().map(ProviderEntry))
            .collect())
    }

    /// Toggle enable/disable through the CLI so the engine applies its own
    /// normalization and permissions.
    pub fn set_enabled(&self, provider_id: &str, enabled: bool) -> Result<()> {
        let sub = if enabled { "enable" } else { "disable" };
        self.run_config(&[sub, "--provider", provider_id])
            .map(|_| ())
    }

    /// Store an API key via the CLI (handles trimming + 0600 perms + enable).
    pub fn set_api_key(&self, provider_id: &str, key: &str, enable: bool) -> Result<()> {
        use std::io::Write;
        let mut args = vec!["config", "set-api-key", "--provider", provider_id, "--stdin"];
        if !enable {
            args.push("--no-enable");
        }
        let mut child = Command::new(&self.engine)
            .args(&args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .spawn()
            .context("spawning set-api-key")?;
        child
            .stdin
            .as_mut()
            .context("set-api-key stdin")?
            .write_all(key.as_bytes())?;
        let status = child.wait()?;
        anyhow::ensure!(status.success(), "set-api-key exited {status}");
        Ok(())
    }

    /// Write per-provider fields the CLI cannot set, editing the JSON directly.
    /// Preserves unknown keys and the providers' order.
    pub fn save_provider_fields(&self, entry: &ProviderEntry) -> Result<()> {
        let text = std::fs::read_to_string(&self.path).unwrap_or_else(|_| {
            r#"{"version":1,"providers":[]}"#.to_string()
        });
        let mut root: Value = serde_json::from_str(&text).context("parsing config")?;
        let arr = root
            .get_mut("providers")
            .and_then(Value::as_array_mut)
            .context("config has no providers array")?;

        let id = entry.id().to_string();
        if let Some(slot) = arr.iter_mut().find(|p| {
            p.get("id").and_then(Value::as_str) == Some(id.as_str())
        }) {
            *slot = Value::Object(entry.0.clone());
        } else {
            arr.push(Value::Object(entry.0.clone()));
        }
        self.write_atomic(&root)
    }

    /// Persist a new provider ordering (array order drives display order).
    pub fn save_order(&self, ordered_ids: &[String]) -> Result<()> {
        let text = std::fs::read_to_string(&self.path)?;
        let mut root: Value = serde_json::from_str(&text)?;
        if let Some(arr) = root.get_mut("providers").and_then(Value::as_array_mut) {
            arr.sort_by_key(|p| {
                let id = p.get("id").and_then(Value::as_str).unwrap_or("");
                ordered_ids.iter().position(|x| x == id).unwrap_or(usize::MAX)
            });
        }
        self.write_atomic(&root)
    }

    pub fn validate(&self) -> Result<String> {
        self.run_config(&["validate", "--format", "json"])
    }

    fn write_atomic(&self, root: &Value) -> Result<()> {
        use std::os::unix::fs::PermissionsExt;
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir).ok();
        }
        let tmp = self.path.with_extension("json.tmp");
        let body = serde_json::to_string_pretty(root)?;
        std::fs::write(&tmp, body.as_bytes())?;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600))?;
        std::fs::rename(&tmp, &self.path)?;
        Ok(())
    }

    fn run_config(&self, args: &[&str]) -> Result<String> {
        let mut full = vec!["config"];
        // `dump`/`validate` etc. are subcommands of `config`; the first arg the
        // caller passes is the subcommand.
        if args.first() == Some(&"config") {
            full.clear();
        }
        full.extend_from_slice(args);
        let out = Command::new(&self.engine)
            .args(&full)
            .output()
            .with_context(|| format!("running codexbar {}", full.join(" ")))?;
        anyhow::ensure!(
            out.status.success(),
            "codexbar {} failed: {}",
            full.join(" "),
            String::from_utf8_lossy(&out.stderr)
        );
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    }
}

fn config_path() -> PathBuf {
    if let Ok(p) = std::env::var("CODEXBAR_CONFIG") {
        return PathBuf::from(p);
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join(".codexbar").join("config.json")
}
