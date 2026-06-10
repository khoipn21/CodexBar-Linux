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

/// Pace label comparing consumption against time elapsed in the window, mirroring
/// the macOS `UsagePaceText`. Returns `None` when the window lacks the timing
/// data needed to project (no `window_minutes` or no `resets_at`).
///
/// Burn ratio = used% / elapsed%. >1 means spending faster than the clock, so
/// the quota will run out before reset; <1 means it will last.
pub fn pace_label(used_percent: f64, window_minutes: Option<i64>, resets_at: Option<&str>) -> Option<String> {
    let window_minutes = window_minutes? as f64;
    if window_minutes <= 0.0 {
        return None;
    }
    let iso = resets_at?;
    let dt = DateTime::parse_from_rfc3339(iso).ok()?.with_timezone(&Utc);
    let remaining_secs = (dt - Utc::now()).num_seconds().max(0) as f64;
    let elapsed_min = (window_minutes - remaining_secs / 60.0).clamp(0.0, window_minutes);
    let elapsed_frac = elapsed_min / window_minutes;
    if elapsed_frac <= 0.01 {
        return None; // too early in the window to project meaningfully
    }
    let used_frac = (used_percent / 100.0).clamp(0.0, 1.0);
    let ratio = used_frac / elapsed_frac;
    let label = if ratio >= 1.25 {
        "ahead of pace — may run out early"
    } else if ratio <= 0.75 {
        "behind pace — quota to spare"
    } else {
        "on pace"
    };
    Some(label.to_string())
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

    #[test]
    fn pace_needs_timing_data() {
        assert!(pace_label(50.0, None, None).is_none());
        assert!(pace_label(50.0, Some(300), None).is_none());
    }

    #[test]
    fn pace_flags_fast_burn() {
        // Window 300m, 90% elapsed remaining => 10% elapsed, but 80% used => ahead.
        let resets = (Utc::now() + chrono::Duration::minutes(270)).to_rfc3339();
        let s = pace_label(80.0, Some(300), Some(&resets)).unwrap();
        assert_eq!(s, "ahead of pace — may run out early");
    }

    #[test]
    fn pace_flags_slow_burn() {
        // 90% elapsed, only 10% used => behind pace.
        let resets = (Utc::now() + chrono::Duration::minutes(30)).to_rfc3339();
        let s = pace_label(10.0, Some(300), Some(&resets)).unwrap();
        assert_eq!(s, "behind pace — quota to spare");
    }
}
