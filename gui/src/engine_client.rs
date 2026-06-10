//! Drives the bundled Swift engine (`codexbar`) and exposes its JSON as typed
//! Rust values. Spawns `codexbar serve` as a child on an ephemeral loopback
//! port, then fetches usage/cost over HTTP.

use crate::model::ProviderPayload;
use anyhow::{anyhow, Context, Result};
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

pub struct EngineClient {
    base_url: String,
    http: reqwest::blocking::Client,
    child: Option<Child>,
}

impl EngineClient {
    /// Locate the engine launcher. Prefers an explicit override, then the
    /// bundled `out/engine/codexbar`, then `codexbar` on PATH.
    pub fn locate_binary() -> Result<PathBuf> {
        if let Ok(p) = std::env::var("CODEXBAR_ENGINE") {
            let pb = PathBuf::from(p);
            if pb.exists() {
                return Ok(pb);
            }
        }
        // Relative to the running binary: ../engine/codexbar (install layout)
        // and the dev layout out/engine/codexbar.
        let mut candidates: Vec<PathBuf> = Vec::new();
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                candidates.push(dir.join("engine/codexbar"));
                candidates.push(dir.join("../engine/codexbar"));
            }
        }
        if let Ok(cwd) = std::env::current_dir() {
            candidates.push(cwd.join("out/engine/codexbar"));
        }
        for c in candidates {
            if c.exists() {
                return Ok(c);
            }
        }
        // Fall back to PATH.
        Ok(PathBuf::from("codexbar"))
    }

    fn pick_free_port() -> Result<u16> {
        let listener = TcpListener::bind("127.0.0.1:0")
            .context("binding ephemeral port")?;
        let port = listener.local_addr()?.port();
        drop(listener);
        Ok(port)
    }

    /// Spawn `codexbar serve` and wait until `/health` responds.
    pub fn spawn(engine: &PathBuf, refresh_interval_secs: u64) -> Result<Self> {
        let port = Self::pick_free_port()?;
        let child = Command::new(engine)
            .arg("serve")
            .arg("--port")
            .arg(port.to_string())
            .arg("--refresh-interval")
            .arg(refresh_interval_secs.to_string())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .with_context(|| format!("spawning {} serve", engine.display()))?;

        let http = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(45))
            .build()?;

        let mut client = EngineClient {
            base_url: format!("http://127.0.0.1:{port}"),
            http,
            child: Some(child),
        };
        client.wait_healthy(Duration::from_secs(15))?;
        Ok(client)
    }

    fn wait_healthy(&self, timeout: Duration) -> Result<()> {
        let url = format!("{}/health", self.base_url);
        let deadline = Instant::now() + timeout;
        loop {
            if let Ok(resp) = self.http.get(&url).send() {
                if resp.status().is_success() {
                    return Ok(());
                }
            }
            if Instant::now() >= deadline {
                return Err(anyhow!("engine did not become healthy within {timeout:?}"));
            }
            std::thread::sleep(Duration::from_millis(250));
        }
    }

    /// Fetch usage for the given provider scope (`None` = enabled providers,
    /// or pass "all" / "both" / a provider id).
    pub fn usage(&self, scope: Option<&str>) -> Result<Vec<ProviderPayload>> {
        let url = match scope {
            Some(s) => format!("{}/usage?provider={}", self.base_url, s),
            None => format!("{}/usage", self.base_url),
        };
        let resp = self.http.get(&url).send().context("GET /usage")?;
        let text = resp.text()?;
        serde_json::from_str(&text)
            .with_context(|| format!("parsing /usage response: {text}"))
    }
}

impl Drop for EngineClient {
    fn drop(&mut self) {
        if let Some(child) = self.child.as_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}
