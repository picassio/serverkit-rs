use chrono::{NaiveDateTime, Utc};

/// Current UTC time formatted exactly like SQLAlchemy stores naive datetimes
/// (`YYYY-MM-DD HH:MM:SS.ffffff`) so rows stay byte-compatible with the
/// Python oracle backend.
pub fn now_sql() -> String {
    Utc::now()
        .naive_utc()
        .format("%Y-%m-%d %H:%M:%S%.6f")
        .to_string()
}

pub fn now_naive() -> NaiveDateTime {
    Utc::now().naive_utc()
}

/// Parse a datetime string as stored by either SQLAlchemy (space separator,
/// optional fractional seconds) or ISO-8601 (`T` separator).
pub fn parse_sql_datetime(s: &str) -> Option<NaiveDateTime> {
    const FORMATS: &[&str] = &[
        "%Y-%m-%d %H:%M:%S%.f",
        "%Y-%m-%dT%H:%M:%S%.f",
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%dT%H:%M:%S",
    ];
    FORMATS
        .iter()
        .find_map(|f| NaiveDateTime::parse_from_str(s, f).ok())
}

/// Convert a stored datetime string to Python `datetime.isoformat()` output
/// (what Flask's `to_dict()` serializers emit).
pub fn to_isoformat(s: &str) -> String {
    s.replacen(' ', "T", 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_both_separators() {
        assert!(parse_sql_datetime("2026-07-04 01:25:00.123456").is_some());
        assert!(parse_sql_datetime("2026-07-04T01:25:00").is_some());
    }

    #[test]
    fn isoformat_matches_python() {
        assert_eq!(
            to_isoformat("2026-07-04 01:25:00.123456"),
            "2026-07-04T01:25:00.123456"
        );
    }
}
