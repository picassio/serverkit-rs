//! Human formatting matching `app/utils/formatting.py` and
//! `SystemService._format_uptime`.

/// `format_bytes(n, precision=1, suffix_sep='')` — 1024-based units, one
/// decimal, no separator: `512.0B`, `1.5KB`, `2.3GB`. Zero renders `0B`.
pub fn format_bytes(n: u64) -> String {
    if n == 0 {
        return "0B".to_string();
    }
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB", "PB"];
    let mut val = n as f64;
    for unit in UNITS {
        if val < 1024.0 {
            return format!("{val:.1}{unit}");
        }
        val /= 1024.0;
    }
    format!("{val:.1}EB")
}

/// `SystemService._format_uptime` — `"3d 4h 12m"`, `'0m'` when < 1 minute.
pub fn format_uptime(total_seconds: u64) -> String {
    let days = total_seconds / 86_400;
    let hours = (total_seconds % 86_400) / 3600;
    let minutes = (total_seconds % 3600) / 60;

    let mut parts = Vec::new();
    if days > 0 {
        parts.push(format!("{days}d"));
    }
    if hours > 0 {
        parts.push(format!("{hours}h"));
    }
    if minutes > 0 {
        parts.push(format!("{minutes}m"));
    }
    if parts.is_empty() {
        "0m".to_string()
    } else {
        parts.join(" ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bytes_match_python() {
        assert_eq!(format_bytes(0), "0B");
        assert_eq!(format_bytes(512), "512.0B");
        assert_eq!(format_bytes(1536), "1.5KB");
    }

    #[test]
    fn uptime_match_python() {
        assert_eq!(format_uptime(30), "0m");
        assert_eq!(format_uptime(86_400 + 3600 + 60), "1d 1h 1m");
        assert_eq!(format_uptime(3600 * 2), "2h");
    }
}
