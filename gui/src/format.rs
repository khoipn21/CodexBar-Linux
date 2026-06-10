//! Formatting helpers for reset countdowns and pace text, mirroring the macOS
//! menu/CLI presentation.

use chrono::{DateTime, Utc};

/// Human countdown to a reset instant, e.g. "resets in 2h 14m", "resets in 3d".
/// Falls back to the engine-provided `reset_description` when no timestamp.
pub fn reset_label(resets_at: Option<&str>, reset_description: Option<&str>) -> Option<String> {
    if let Some(iso) = resets_at {
        if let Ok(dt) = DateTime::parse_from_rfc3339(iso) {
            let dt_utc = dt.with_timezone(&Utc);
            let now = Utc::now();
            let secs = (dt_utc - now).num_seconds();
            if secs <= 0 {
                return Some("resetting now".to_string());
            }
            return Some(format!("resets in {}", humanize_secs(secs)));
        }
    }
    reset_description.map(|d| format!("resets {d}"))
}

fn humanize_secs(secs: i64) -> String {
    let days = secs / 86_400;
    let hours = (secs % 86_400) / 3_600;
    let mins = (secs % 3_600) / 60;
    if days > 0 {
        if hours > 0 {
            format!("{days}d {hours}h")
        } else {
            format!("{days}d")
        }
    } else if hours > 0 {
        format!("{hours}h {mins}m")
    } else if mins > 0 {
        format!("{mins}m")
    } else {
        "<1m".to_string()
    }
}

/// "72% left" style label from remaining percent.
pub fn remaining_label(remaining_percent: f64) -> String {
    format!("{}% left", remaining_percent.round() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_future_reset() {
        let future = (Utc::now() + chrono::Duration::seconds(2 * 3600 + 14 * 60))
            .to_rfc3339();
        let s = reset_label(Some(&future), None).unwrap();
        assert!(s.starts_with("resets in 2h"), "got {s}");
    }

    #[test]
    fn falls_back_to_description() {
        let s = reset_label(None, Some("Fri at 9:00 AM")).unwrap();
        assert_eq!(s, "resets Fri at 9:00 AM");
    }

    #[test]
    fn past_reset_is_now() {
        let past = (Utc::now() - chrono::Duration::seconds(5)).to_rfc3339();
        assert_eq!(reset_label(Some(&past), None).unwrap(), "resetting now");
    }
}
