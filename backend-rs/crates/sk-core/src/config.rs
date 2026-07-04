use std::env;

/// Mirrors the env contract of the Flask backend (`backend/config.py`) so the
/// Rust server is a drop-in replacement: same variable names, same defaults.
#[derive(Debug, Clone)]
pub struct Config {
    /// Filesystem path to the SQLite database (parsed from `DATABASE_URL`).
    pub database_path: String,
    pub secret_key: String,
    pub jwt_secret_key: String,
    /// Access token TTL in seconds (Flask: 15 minutes).
    pub jwt_access_ttl_secs: i64,
    /// Refresh token TTL in seconds (Flask: 30 days).
    pub jwt_refresh_ttl_secs: i64,
    pub port: u16,
    /// Directory containing the built React frontend (served statically).
    pub frontend_dist: String,
}

const INSECURE_KEYS: &[&str] = &[
    "dev-secret-key-change-in-production",
    "jwt-secret-key-change-in-production",
    "change-this-to-a-secure-random-string",
    "change-this-to-another-secure-random-string",
];

impl Config {
    pub fn from_env() -> Self {
        let database_url = env::var("DATABASE_URL")
            .unwrap_or_else(|_| "sqlite:///app/instance/serverkit.db".to_string());
        let database_path = parse_sqlite_url(&database_url);

        let secret_key = env::var("SECRET_KEY")
            .unwrap_or_else(|_| "dev-secret-key-change-in-production".to_string());
        let jwt_secret_key = env::var("JWT_SECRET_KEY")
            .unwrap_or_else(|_| "jwt-secret-key-change-in-production".to_string());

        if INSECURE_KEYS.contains(&jwt_secret_key.as_str()) {
            tracing::warn!("JWT_SECRET_KEY is an insecure default — change it in production");
        }

        Self {
            database_path,
            secret_key,
            jwt_secret_key,
            jwt_access_ttl_secs: 15 * 60,
            jwt_refresh_ttl_secs: 30 * 24 * 3600,
            port: env::var("PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(5000),
            frontend_dist: env::var("SK_FRONTEND_DIST")
                .unwrap_or_else(|_| "../frontend/dist".to_string()),
        }
    }
}

/// Convert a SQLAlchemy-style sqlite URL (`sqlite:////abs/path`, `sqlite:///rel/path`)
/// into a plain filesystem path.
fn parse_sqlite_url(url: &str) -> String {
    let rest = url
        .strip_prefix("sqlite:///")
        .or_else(|| url.strip_prefix("sqlite://"))
        .or_else(|| url.strip_prefix("sqlite:"))
        .unwrap_or(url);
    // SQLAlchemy uses 4 slashes for absolute paths: sqlite:////abs → /abs
    if let Some(abs) = rest.strip_prefix('/') {
        if url.starts_with("sqlite:////") {
            return format!("/{abs}");
        }
    }
    rest.to_string()
}

#[cfg(test)]
mod tests {
    use super::parse_sqlite_url;

    #[test]
    fn parses_sqlalchemy_urls() {
        assert_eq!(
            parse_sqlite_url("sqlite:////app/instance/db.db"),
            "/app/instance/db.db"
        );
        assert_eq!(
            parse_sqlite_url("sqlite:///instance/db.db"),
            "instance/db.db"
        );
        assert_eq!(parse_sqlite_url("/tmp/x.db"), "/tmp/x.db");
    }
}
