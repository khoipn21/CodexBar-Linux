//! Linux web/cookie support: import browser cookies and turn them into the
//! `cookieHeader` the engine consumes. See docs/linux-web-provider-triage.md.

pub mod cookie_import;

use anyhow::{anyhow, Result};
use cookie_import::{Cookie, CHROMIUM_BROWSERS};

/// Cookie domains + (optional) wanted cookie names per provider id, for the
/// cookie-only (category-2) providers. Empty `names` = take all cookies for the
/// domain.
struct ProviderCookieSpec {
    domains: &'static [&'static str],
    names: &'static [&'static str],
}

fn spec_for(provider_id: &str) -> Option<ProviderCookieSpec> {
    let s = match provider_id {
        "cursor" => ProviderCookieSpec {
            domains: &["cursor.com", "cursor.sh"],
            names: &[
                "WorkosCursorSessionToken",
                "__Secure-next-auth.session-token",
                "next-auth.session-token",
                "wos-session",
                "__Secure-wos-session",
            ],
        },
        "opencode" | "opencodego" => ProviderCookieSpec { domains: &["opencode.ai"], names: &[] },
        "amp" => ProviderCookieSpec { domains: &["ampcode.com", "sourcegraph.com"], names: &[] },
        "manus" => ProviderCookieSpec { domains: &["manus.im"], names: &["session_id"] },
        "mistral" => ProviderCookieSpec { domains: &["mistral.ai", "console.mistral.ai"], names: &[] },
        "abacus" => ProviderCookieSpec { domains: &["abacus.ai"], names: &[] },
        "commandcode" => ProviderCookieSpec { domains: &["commandcode.ai"], names: &[] },
        "mimo" => ProviderCookieSpec { domains: &["xiaomi.com", "mimo.xiaomi.com"], names: &[] },
        "perplexity" => ProviderCookieSpec { domains: &["perplexity.ai"], names: &[] },
        "stepfun" => ProviderCookieSpec { domains: &["stepfun.com"], names: &[] },
        "t3chat" => ProviderCookieSpec { domains: &["t3.chat"], names: &[] },
        "kimi" => ProviderCookieSpec { domains: &["kimi.com", "moonshot.cn"], names: &[] },
        "grok" => ProviderCookieSpec { domains: &["grok.com", "x.ai"], names: &[] },
        "alibaba" => ProviderCookieSpec { domains: &["aliyun.com", "bailian.console.aliyun.com"], names: &[] },
        "minimax" => ProviderCookieSpec { domains: &["minimax.io", "minimaxi.com"], names: &[] },
        "ollama" => ProviderCookieSpec { domains: &["ollama.com"], names: &[] },
        "windsurf" => ProviderCookieSpec { domains: &["windsurf.com", "codeium.com"], names: &[] },
        _ => return None,
    };
    Some(s)
}

/// True if the provider has a known cookie spec (i.e. automatic import is
/// meaningful for it on Linux).
pub fn supports_cookie_import(provider_id: &str) -> bool {
    spec_for(provider_id).is_some()
}

/// Import cookies for a provider from all available browsers and build a header.
/// Tries each Chromium-family browser, then Firefox; returns the first match.
pub fn import_cookie_header(provider_id: &str) -> Result<String> {
    let spec = spec_for(provider_id)
        .ok_or_else(|| anyhow!("no cookie spec for provider {provider_id}"))?;

    let mut all: Vec<Cookie> = Vec::new();
    for browser in CHROMIUM_BROWSERS {
        match cookie_import::import_chromium(browser) {
            Ok(mut c) => all.append(&mut c),
            Err(e) => log::debug!("{}: {e}", browser.name),
        }
    }
    if let Ok(mut ff) = cookie_import::import_firefox() {
        all.append(&mut ff);
    }

    cookie_import::cookie_header_for(spec.domains, spec.names, &all)
        .ok_or_else(|| anyhow!("no matching cookies found for {provider_id} in any browser"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_cookie_providers_have_specs() {
        assert!(supports_cookie_import("cursor"));
        assert!(supports_cookie_import("perplexity"));
        assert!(!supports_cookie_import("openai")); // API provider, no cookie import
    }
}
