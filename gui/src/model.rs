//! Rust mirrors of the CodexBar engine JSON contract.
//!
//! Source of truth: engine/CodexBar/Sources/CodexBarCLI/CLIPayloads.swift and
//! the `codexbar serve` endpoints. See docs/system-architecture.md.
//!
//! The model is intentionally permissive: unknown fields are ignored so an
//! upstream engine bump does not break deserialization, and every window key is
//! optional because the engine encodes absent windows as `null`.

use serde::Deserialize;

/// One provider's full payload from `/usage` or `codexbar usage --format json`.
#[derive(Debug, Clone, Deserialize)]
pub struct ProviderPayload {
    pub provider: String,
    #[serde(default)]
    pub account: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub status: Option<ProviderStatus>,
    #[serde(default)]
    pub usage: Option<UsageSnapshot>,
    #[serde(default)]
    pub credits: Option<CreditsSnapshot>,
    #[serde(default)]
    pub error: Option<ProviderError>,
}

impl ProviderPayload {
    /// True when the engine returned an error instead of usage (e.g. the
    /// macOS-web-only ceiling on Linux).
    pub fn is_error(&self) -> bool {
        self.error.is_some()
    }

    /// The window that best represents this provider for the tray meter:
    /// primary if present, else the first available window.
    pub fn headline_window(&self) -> Option<&RateWindow> {
        let u = self.usage.as_ref()?;
        u.primary
            .as_ref()
            .or(u.secondary.as_ref())
            .or(u.tertiary.as_ref())
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProviderError {
    #[serde(default)]
    pub code: Option<i32>,
    #[serde(default)]
    pub kind: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProviderStatus {
    /// none | minor | major | critical | maintenance | unknown
    pub indicator: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(rename = "updatedAt", default)]
    pub updated_at: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
}

impl ProviderStatus {
    pub fn is_incident(&self) -> bool {
        matches!(self.indicator.as_str(), "minor" | "major" | "critical")
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct UsageSnapshot {
    #[serde(default)]
    pub primary: Option<RateWindow>,
    #[serde(default)]
    pub secondary: Option<RateWindow>,
    #[serde(default)]
    pub tertiary: Option<RateWindow>,
    #[serde(rename = "extraRateWindows", default)]
    pub extra_rate_windows: Option<Vec<NamedRateWindow>>,
    #[serde(default)]
    pub identity: Option<ProviderIdentity>,
    #[serde(rename = "updatedAt", default)]
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RateWindow {
    #[serde(rename = "usedPercent")]
    pub used_percent: f64,
    #[serde(rename = "windowMinutes", default)]
    pub window_minutes: Option<i64>,
    #[serde(rename = "resetsAt", default)]
    pub resets_at: Option<String>,
    #[serde(rename = "resetDescription", default)]
    pub reset_description: Option<String>,
    #[serde(rename = "nextRegenPercent", default)]
    pub next_regen_percent: Option<f64>,
}

impl RateWindow {
    pub fn remaining_percent(&self) -> f64 {
        (100.0 - self.used_percent).clamp(0.0, 100.0)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct NamedRateWindow {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(flatten)]
    pub window: RateWindow,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProviderIdentity {
    #[serde(rename = "providerID", default)]
    pub provider_id: Option<String>,
    #[serde(rename = "accountEmail", default)]
    pub account_email: Option<String>,
    #[serde(rename = "accountOrganization", default)]
    pub account_organization: Option<String>,
    #[serde(rename = "loginMethod", default)]
    pub login_method: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreditsSnapshot {
    pub remaining: f64,
    #[serde(default)]
    pub events: Vec<CreditEvent>,
    #[serde(rename = "updatedAt", default)]
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreditEvent {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub date: Option<String>,
    #[serde(default)]
    pub service: Option<String>,
    #[serde(rename = "creditsUsed", default)]
    pub credits_used: Option<f64>,
}

/// One provider's cost/token spend from `/cost` or `codexbar cost --format json`.
/// Local-only (token-ledger derived); only Claude and Codex report data.
/// Source of truth: engine CLICostCommand.swift `CostPayload`.
#[derive(Debug, Clone, Deserialize)]
pub struct CostPayload {
    pub provider: String,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(rename = "currencyCode", default)]
    pub currency_code: Option<String>,
    #[serde(rename = "sessionTokens", default)]
    pub session_tokens: Option<i64>,
    #[serde(rename = "sessionCostUSD", default)]
    pub session_cost_usd: Option<f64>,
    #[serde(rename = "historyDays", default)]
    pub history_days: Option<i64>,
    #[serde(rename = "last30DaysTokens", default)]
    pub last_30_days_tokens: Option<i64>,
    #[serde(rename = "last30DaysCostUSD", default)]
    pub last_30_days_cost_usd: Option<f64>,
    #[serde(default)]
    pub daily: Vec<CostDailyEntry>,
    #[serde(default)]
    pub totals: Option<CostTotals>,
    #[serde(rename = "updatedAt", default)]
    pub updated_at: Option<String>,
    #[serde(default)]
    pub error: Option<ProviderError>,
}

impl CostPayload {
    /// Currency code with a sensible default for formatting.
    pub fn currency(&self) -> &str {
        self.currency_code.as_deref().unwrap_or("USD")
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct CostDailyEntry {
    pub date: String,
    #[serde(rename = "inputTokens", default)]
    pub input_tokens: Option<i64>,
    #[serde(rename = "outputTokens", default)]
    pub output_tokens: Option<i64>,
    #[serde(rename = "cacheReadTokens", default)]
    pub cache_read_tokens: Option<i64>,
    #[serde(rename = "cacheCreationTokens", default)]
    pub cache_creation_tokens: Option<i64>,
    #[serde(rename = "totalTokens", default)]
    pub total_tokens: Option<i64>,
    #[serde(rename = "totalCost", default)]
    pub cost_usd: Option<f64>,
    #[serde(rename = "modelsUsed", default)]
    pub models_used: Option<Vec<String>>,
    #[serde(rename = "modelBreakdowns", default)]
    pub model_breakdowns: Option<Vec<CostModelBreakdown>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CostModelBreakdown {
    #[serde(rename = "modelName")]
    pub model_name: String,
    #[serde(rename = "cost", default)]
    pub cost_usd: Option<f64>,
    #[serde(rename = "totalTokens", default)]
    pub total_tokens: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CostTotals {
    #[serde(rename = "inputTokens", default)]
    pub total_input_tokens: Option<i64>,
    #[serde(rename = "outputTokens", default)]
    pub total_output_tokens: Option<i64>,
    #[serde(rename = "cacheReadTokens", default)]
    pub cache_read_tokens: Option<i64>,
    #[serde(rename = "cacheCreationTokens", default)]
    pub cache_creation_tokens: Option<i64>,
    #[serde(rename = "totalTokens", default)]
    pub total_tokens: Option<i64>,
    #[serde(rename = "totalCost", default)]
    pub total_cost_usd: Option<f64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> String {
        let path = format!(
            "{}/tests/fixtures/{}",
            env!("CARGO_MANIFEST_DIR"),
            name
        );
        std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("read {path}: {e}"))
    }

    #[test]
    fn deserializes_healthy_codex() {
        let payloads: Vec<ProviderPayload> =
            serde_json::from_str(&fixture("usage-codex-healthy.json")).unwrap();
        assert_eq!(payloads.len(), 1);
        let p = &payloads[0];
        assert_eq!(p.provider, "codex");
        assert!(!p.is_error());
        let primary = p.headline_window().unwrap();
        assert_eq!(primary.used_percent, 1.0);
        assert_eq!(primary.remaining_percent(), 99.0);
        assert_eq!(primary.window_minutes, Some(300));
        let usage = p.usage.as_ref().unwrap();
        assert_eq!(usage.secondary.as_ref().unwrap().used_percent, 3.0);
        assert_eq!(
            usage.identity.as_ref().unwrap().login_method.as_deref(),
            Some("plus")
        );
    }

    #[test]
    fn deserializes_web_ceiling_error() {
        let payloads: Vec<ProviderPayload> =
            serde_json::from_str(&fixture("usage-claude-web-ceiling.json")).unwrap();
        let p = &payloads[0];
        assert_eq!(p.provider, "claude");
        assert!(p.is_error());
        assert!(p.error.as_ref().unwrap().message.contains("macOS"));
        assert!(p.headline_window().is_none());
    }

    #[test]
    fn deserializes_default_usage() {
        // Should parse without panicking even though it carries an error payload.
        let _: Vec<ProviderPayload> =
            serde_json::from_str(&fixture("usage-default.json")).unwrap();
    }

    #[test]
    fn deserializes_codex_cost() {
        let payloads: Vec<CostPayload> =
            serde_json::from_str(&fixture("cost-codex.json")).unwrap();
        assert_eq!(payloads.len(), 1);
        let c = &payloads[0];
        assert_eq!(c.provider, "codex");
        assert_eq!(c.currency(), "USD");
        assert_eq!(c.session_tokens, Some(39152164));
        assert_eq!(c.last_30_days_cost_usd, Some(161.8174724));
        assert_eq!(c.daily.len(), 11);
        let first = &c.daily[0];
        assert_eq!(first.date, "2026-05-30");
        assert_eq!(first.cost_usd, Some(5.312425));
        let breakdown = first.model_breakdowns.as_ref().unwrap();
        assert_eq!(breakdown[0].model_name, "gpt-5.5");
        let totals = c.totals.as_ref().unwrap();
        assert_eq!(totals.total_tokens, Some(289257016));
    }

    #[test]
    fn deserializes_default_cost() {
        // Default/empty cost should parse without panicking.
        let _: Vec<CostPayload> =
            serde_json::from_str(&fixture("cost-default.json")).unwrap();
    }
}
