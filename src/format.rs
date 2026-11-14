//! Small display helpers shared across commands and hooks.

use std::time::Duration;

/// Format milliseconds as `1:23:45` or `3:07`.
pub fn fmt_ms(ms: u64) -> String {
    let total = ms / 1000;
    let (h, m, s) = (total / 3600, (total % 3600) / 60, total % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

/// Parse `90`, `1:30`, `1m30s`, `2h` into a Duration.
pub fn parse_time(input: &str) -> Option<Duration> {
    let s = input.trim().to_ascii_lowercase();

    if let Ok(secs) = s.parse::<u64>() {
        return Some(Duration::from_secs(secs));
    }

    if let Some((m, sec)) = s.split_once(':') {
        let m: u64 = m.parse().ok()?;
        let sec: u64 = sec.parse().ok()?;
        if sec >= 60 {
            return None;
        }
        return Some(Duration::from_secs(m * 60 + sec));
    }

    // 1h2m3s style
    let mut total = 0u64;
    let mut num = String::new();
    let mut matched = false;
    for ch in s.chars() {
        if ch.is_ascii_digit() {
            num.push(ch);
            continue;
        }
        let value: u64 = num.parse().ok()?;
        num.clear();
        total += match ch {
            'h' => value * 3600,
            'm' => value * 60,
            's' => value,
            _ => return None,
        };
        matched = true;
    }
    if matched && num.is_empty() {
        Some(Duration::from_secs(total))
    } else {
        None
    }
}

/// Trim to `max` chars, adding an ellipsis.
pub fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let cut: String = s.chars().take(max.saturating_sub(1)).collect();
        format!("{cut}…")
    }
}

/// Simple progress bar, e.g. `▬▬▬🔘▬▬▬▬▬▬`.
pub fn progress_bar(position: u64, length: u64, width: usize) -> String {
    if length == 0 {
        return "🔘".to_string();
    }
    let ratio = (position as f64 / length as f64).clamp(0.0, 1.0);
    let idx = ((ratio * width as f64) as usize).min(width.saturating_sub(1));
    let mut bar = String::with_capacity(width * 4);
    for i in 0..width {
        bar.push_str(if i == idx { "🔘" } else { "▬" });
    }
    bar
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_time_formats() {
        assert_eq!(parse_time("90"), Some(Duration::from_secs(90)));
        assert_eq!(parse_time("1:30"), Some(Duration::from_secs(90)));
        assert_eq!(parse_time("1m30s"), Some(Duration::from_secs(90)));
        assert_eq!(parse_time("2h"), Some(Duration::from_secs(7200)));
        assert_eq!(parse_time("abc"), None);
        assert_eq!(parse_time("1:90"), None);
    }

    #[test]
    fn formats_ms() {
        assert_eq!(fmt_ms(0), "0:00");
        assert_eq!(fmt_ms(187_000), "3:07");
        assert_eq!(fmt_ms(3_725_000), "1:02:05");
    }
}
