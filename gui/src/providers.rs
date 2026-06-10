//! Provider id -> display name + accent color, mirroring the macOS provider
//! descriptors' branding. The engine emits provider ids; the GUI maps them to
//! a friendly label and an accent color for cards and the tray meter.
//!
//! Unknown ids fall back to a title-cased label and a deterministic color so a
//! newly added upstream provider still renders sensibly.

/// RGB in 0.0..=1.0, the form cairo and GTK want.
#[derive(Debug, Clone, Copy)]
pub struct Rgb {
    pub r: f64,
    pub g: f64,
    pub b: f64,
}

impl Rgb {
    const fn new(r: f64, g: f64, b: f64) -> Self {
        Rgb { r, g, b }
    }
}

pub struct ProviderBranding {
    pub display_name: String,
    pub color: Rgb,
}

/// Friendly display names for known provider ids (from engine descriptors).
fn display_name_for(id: &str) -> Option<&'static str> {
    Some(match id {
        "codex" => "Codex",
        "openai" => "OpenAI",
        "azureopenai" => "Azure OpenAI",
        "claude" => "Claude",
        "cursor" => "Cursor",
        "opencode" => "OpenCode",
        "opencodego" => "OpenCode Go",
        "alibaba" => "Alibaba Coding Plan",
        "alibabatokenplan" => "Alibaba Token Plan",
        "factory" => "Droid",
        "gemini" => "Gemini",
        "antigravity" => "Antigravity",
        "copilot" => "Copilot",
        "zai" => "z.ai",
        "minimax" => "MiniMax",
        "manus" => "Manus",
        "kimi" => "Kimi",
        "kilo" => "Kilo",
        "kiro" => "Kiro",
        "vertexai" => "Vertex AI",
        "augment" => "Augment",
        "jetbrains" => "JetBrains AI",
        "kimik2" => "Kimi K2",
        "moonshot" => "Moonshot",
        "amp" => "Amp",
        "t3chat" => "T3 Chat",
        "ollama" => "Ollama",
        "synthetic" => "Synthetic",
        "warp" => "Warp",
        "openrouter" => "OpenRouter",
        "elevenlabs" => "ElevenLabs",
        "windsurf" => "Windsurf",
        "perplexity" => "Perplexity",
        "mimo" => "Xiaomi MiMo",
        "doubao" => "Doubao",
        "abacus" => "Abacus AI",
        "mistral" => "Mistral",
        "deepseek" => "DeepSeek",
        "codebuff" => "Codebuff",
        "crof" => "Crof",
        "venice" => "Venice",
        "commandcode" => "Command Code",
        "stepfun" => "StepFun",
        "bedrock" => "AWS Bedrock",
        "grok" => "Grok",
        "groq" => "GroqCloud",
        "llmproxy" => "LLM Proxy",
        "deepgram" => "Deepgram",
        _ => return None,
    })
}

/// Accent colors for the most common providers; others get a hashed color.
fn color_for(id: &str) -> Rgb {
    match id {
        "codex" | "openai" => Rgb::new(0.04, 0.04, 0.05),
        "claude" => Rgb::new(0.85, 0.47, 0.27),
        "cursor" => Rgb::new(0.20, 0.20, 0.22),
        "gemini" => Rgb::new(0.26, 0.52, 0.96),
        "copilot" => Rgb::new(0.18, 0.18, 0.20),
        "grok" => Rgb::new(0.10, 0.10, 0.12),
        "zai" => Rgb::new(0.30, 0.40, 0.95),
        "minimax" => Rgb::new(0.90, 0.30, 0.30),
        "mistral" => Rgb::new(0.95, 0.45, 0.10),
        "deepseek" => Rgb::new(0.29, 0.42, 0.93),
        "elevenlabs" => Rgb::new(0.10, 0.10, 0.10),
        "openrouter" => Rgb::new(0.40, 0.35, 0.85),
        _ => hashed_color(id),
    }
}

/// Deterministic pleasant color from the id (HSV-ish via hue rotation).
fn hashed_color(id: &str) -> Rgb {
    let mut h: u32 = 2166136261;
    for b in id.bytes() {
        h ^= b as u32;
        h = h.wrapping_mul(16777619);
    }
    let hue = (h % 360) as f64 / 360.0;
    hsv_to_rgb(hue, 0.55, 0.75)
}

fn hsv_to_rgb(h: f64, s: f64, v: f64) -> Rgb {
    let i = (h * 6.0).floor();
    let f = h * 6.0 - i;
    let p = v * (1.0 - s);
    let q = v * (1.0 - f * s);
    let t = v * (1.0 - (1.0 - f) * s);
    let (r, g, b) = match (i as i32) % 6 {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    Rgb::new(r, g, b)
}

fn title_case(id: &str) -> String {
    let mut c = id.chars();
    match c.next() {
        Some(first) => first.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

pub fn branding(id: &str) -> ProviderBranding {
    ProviderBranding {
        display_name: display_name_for(id)
            .map(|s| s.to_string())
            .unwrap_or_else(|| title_case(id)),
        color: color_for(id),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_provider_has_friendly_name() {
        assert_eq!(branding("codex").display_name, "Codex");
        assert_eq!(branding("zai").display_name, "z.ai");
    }

    #[test]
    fn unknown_provider_falls_back() {
        let b = branding("somenewthing");
        assert_eq!(b.display_name, "Somenewthing");
        // hashed color is deterministic
        let b2 = branding("somenewthing");
        assert_eq!(b.color.r, b2.color.r);
    }
}
