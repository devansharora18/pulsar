//! Human-friendly formatting. Kept pure so it is easy to test.

/// Bytes → "1.23 GB" (binary units).
pub fn human_bytes(bytes: f64) -> String {
    const UNITS: [&str; 6] = ["B", "KB", "MB", "GB", "TB", "PB"];
    let mut v = bytes.max(0.0);
    let mut i = 0;
    while v >= 1024.0 && i < UNITS.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 || v >= 100.0 {
        format!("{v:.0} {}", UNITS[i])
    } else {
        format!("{v:.1} {}", UNITS[i])
    }
}

/// Kibibytes → human bytes.
pub fn human_kb(kb: u64) -> String {
    human_bytes(kb as f64 * 1024.0)
}

/// Bytes per second → "1.2 MB/s".
pub fn human_rate(bytes_per_s: f64) -> String {
    format!("{}/s", human_bytes(bytes_per_s))
}

/// Watts, dropping to milliwatts below 1 W.
pub fn human_watts(w: f64) -> String {
    if !w.is_finite() {
        return "—".to_string();
    }
    if w.abs() < 1.0 {
        format!("{:.0} mW", w * 1000.0)
    } else if w < 100.0 {
        format!("{w:.1} W")
    } else {
        format!("{w:.0} W")
    }
}

/// Percentage with one decimal.
pub fn pct(v: f64) -> String {
    format!("{v:.1}%")
}

/// Frequency in MHz → "3.42 GHz" or "850 MHz".
pub fn human_freq_mhz(mhz: u64) -> String {
    if mhz >= 1000 {
        format!("{:.2} GHz", mhz as f64 / 1000.0)
    } else {
        format!("{mhz} MHz")
    }
}

/// Temperature with one decimal.
pub fn human_temp(c: f64) -> String {
    format!("{c:.1}°C")
}

/// Seconds → "3h 12m" / "12m 04s" / "42s".
#[allow(dead_code)]
pub fn human_duration(secs: f64) -> String {
    let s = secs.max(0.0) as u64;
    let (h, m, sec) = (s / 3600, (s % 3600) / 60, s % 60);
    if h > 0 {
        format!("{h}h {m:02}m")
    } else if m > 0 {
        format!("{m}m {sec:02}s")
    } else {
        format!("{sec}s")
    }
}

/// A monogram (first letter, uppercase) for entries with no icon.
pub fn monogram(name: &str) -> String {
    name.chars()
        .find(|c| c.is_alphanumeric())
        .map(|c| c.to_uppercase().to_string())
        .unwrap_or_else(|| "?".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bytes_scale() {
        assert_eq!(human_bytes(512.0), "512 B");
        assert_eq!(human_bytes(1024.0), "1.0 KB");
        assert_eq!(human_bytes(1536.0), "1.5 KB");
        assert_eq!(human_bytes(1024.0 * 1024.0 * 1024.0 * 10.3), "10.3 GB");
    }

    #[test]
    fn watts_switch_units() {
        assert_eq!(human_watts(0.25), "250 mW");
        assert_eq!(human_watts(12.34), "12.3 W");
        assert_eq!(human_watts(123.4), "123 W");
        assert_eq!(human_watts(f64::NAN), "—");
    }

    #[test]
    fn freq_and_temp() {
        assert_eq!(human_freq_mhz(3420), "3.42 GHz");
        assert_eq!(human_freq_mhz(850), "850 MHz");
        assert_eq!(human_temp(41.375), "41.4°C");
    }

    #[test]
    fn durations() {
        assert_eq!(human_duration(42.0), "42s");
        assert_eq!(human_duration(724.0), "12m 04s");
        assert_eq!(human_duration(11520.0), "3h 12m");
    }

    #[test]
    fn monogram_picks_first_alnum() {
        assert_eq!(monogram("firefox"), "F");
        assert_eq!(monogram("_zen"), "Z");
        assert_eq!(monogram(""), "?");
    }
}
