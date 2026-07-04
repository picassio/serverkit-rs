//! Store **database** backups — dump/restore the store's Dockerized MariaDB.
//! Files land in `$SK_MAGENTO_BACKUP_DIR/<store>/magento_<ts>.sql.gz`.
//!
//! DB-only by design: media/code are on disk and versioned/deployable
//! separately; the durable state that matters is the database.

use crate::store::Store;
use serde_json::{json, Value};
use std::process::Stdio;
use tokio::process::Command;

fn backup_root() -> String {
    std::env::var("SK_MAGENTO_BACKUP_DIR")
        .unwrap_or_else(|_| "/var/backups/serverkit/magento".into())
}

fn backup_dir(name: &str) -> String {
    format!("{}/{name}", backup_root())
}

fn container(name: &str) -> String {
    format!("magento-{name}-db")
}

/// filename must be a bare `*.sql[.gz]` in the store's backup dir.
fn valid_filename(f: &str) -> bool {
    !f.is_empty()
        && !f.contains('/')
        && !f.contains("..")
        && (f.ends_with(".sql.gz") || f.ends_with(".sql"))
}

/// Resolve a tool that exists inside the DB container (mariadb 11 dropped the
/// `mysql*` names; 10.x ships both).
async fn container_tool(container: &str, candidates: &[&str]) -> Option<String> {
    let probe = candidates
        .iter()
        .map(|c| format!("command -v {c}"))
        .collect::<Vec<_>>()
        .join(" || ");
    let out = Command::new("docker")
        .args(["exec", container, "sh", "-c", &probe])
        .output()
        .await
        .ok()?;
    if out.status.success() {
        let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
        (!path.is_empty()).then(|| path.rsplit('/').next().unwrap_or(&path).to_string())
    } else {
        None
    }
}

fn human_size(bytes: u64) -> String {
    let mut v = bytes as f64;
    for u in ["B", "KB", "MB", "GB"] {
        if v < 1024.0 {
            return format!("{v:.1} {u}");
        }
        v /= 1024.0;
    }
    format!("{v:.1} TB")
}

/// Create a gzipped SQL dump of the store's `magento` database.
pub async fn backup_db(s: &Store) -> Value {
    let cname = container(&s.name);
    let dir = backup_dir(&s.name);
    if let Err(e) = std::fs::create_dir_all(&dir) {
        return json!({ "success": false, "error": e.to_string() });
    }
    let Some(tool) = container_tool(&cname, &["mariadb-dump", "mysqldump"]).await else {
        return json!({ "success": false, "error": "no dump tool in the DB container (is it running?)" });
    };
    let pw = s.db_password_plain().unwrap_or_default();
    let ts = chrono::Local::now().format("%Y%m%d_%H%M%S");
    let path = format!("{dir}/magento_{ts}.sql.gz");

    // docker exec ... <tool> --single-transaction --quick magento  |  gzip -c > file
    let mut dump = Command::new("docker");
    dump.args([
        "exec",
        "-e",
        &format!("MYSQL_PWD={pw}"),
        &cname,
        &tool,
        "-u",
        "root",
        "--single-transaction",
        "--quick",
        "--routines",
        "magento",
    ])
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());

    let mut dump_child = match dump.spawn() {
        Ok(c) => c,
        Err(e) => return json!({ "success": false, "error": e.to_string() }),
    };
    let dump_out: Stdio = match dump_child.stdout.take().map(|s| s.try_into()) {
        Some(Ok(s)) => s,
        _ => return json!({ "success": false, "error": "failed to pipe dump" }),
    };
    let outfile = match std::fs::File::create(&path) {
        Ok(f) => f,
        Err(e) => return json!({ "success": false, "error": e.to_string() }),
    };
    let gzip = Command::new("gzip")
        .arg("-c")
        .stdin(dump_out)
        .stdout(Stdio::from(outfile))
        .status();

    let (dump_status, gzip_status) = tokio::join!(dump_child.wait(), gzip);
    let ok = dump_status.map(|s| s.success()).unwrap_or(false)
        && gzip_status.map(|s| s.success()).unwrap_or(false);

    if ok {
        let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        json!({ "success": true, "path": path, "size": size, "size_human": human_size(size) })
    } else {
        let mut err = String::new();
        if let Some(mut e) = dump_child.stderr.take() {
            use tokio::io::AsyncReadExt;
            let _ = e.read_to_string(&mut err).await;
        }
        let _ = std::fs::remove_file(&path);
        json!({ "success": false, "error": if err.trim().is_empty() { "backup failed".into() } else { err } })
    }
}

/// List a store's backups, newest first.
pub fn list_backups(s: &Store) -> Vec<Value> {
    let dir = backup_dir(&s.name);
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut out: Vec<Value> = entries
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            if !valid_filename(&name) {
                return None;
            }
            let meta = e.metadata().ok()?;
            let created = meta
                .modified()
                .ok()
                .map(|t| {
                    chrono::DateTime::<chrono::Local>::from(t)
                        .naive_local()
                        .format("%Y-%m-%dT%H:%M:%S")
                        .to_string()
                })
                .unwrap_or_default();
            Some(json!({
                "filename": name,
                "size": meta.len(),
                "size_human": human_size(meta.len()),
                "created_at": created,
            }))
        })
        .collect();
    out.sort_by(|a, b| {
        b["filename"]
            .as_str()
            .unwrap_or("")
            .cmp(a["filename"].as_str().unwrap_or(""))
    });
    out
}

/// Restore a backup into the store's `magento` database (gunzip | client).
pub async fn restore_db(s: &Store, filename: &str) -> Value {
    if !valid_filename(filename) {
        return json!({ "success": false, "error": "invalid backup filename" });
    }
    let path = format!("{}/{filename}", backup_dir(&s.name));
    if !std::path::Path::new(&path).exists() {
        return json!({ "success": false, "error": "backup not found" });
    }
    let cname = container(&s.name);
    let Some(client) = container_tool(&cname, &["mariadb", "mysql"]).await else {
        return json!({ "success": false, "error": "no mysql client in the DB container" });
    };
    let pw = s.db_password_plain().unwrap_or_default();

    // gunzip -c file (or cat)  |  docker exec -i ... <client> magento
    let reader: Stdio = if filename.ends_with(".gz") {
        let mut gunzip = match Command::new("gunzip")
            .args(["-c", &path])
            .stdout(Stdio::piped())
            .spawn()
        {
            Ok(c) => c,
            Err(e) => return json!({ "success": false, "error": e.to_string() }),
        };
        match gunzip.stdout.take().map(|s| s.try_into()) {
            Some(Ok(s)) => s,
            _ => return json!({ "success": false, "error": "failed to pipe gunzip" }),
        }
    } else {
        match std::fs::File::open(&path) {
            Ok(f) => Stdio::from(f),
            Err(e) => return json!({ "success": false, "error": e.to_string() }),
        }
    };

    let out = Command::new("docker")
        .args([
            "exec",
            "-i",
            "-e",
            &format!("MYSQL_PWD={pw}"),
            &cname,
            &client,
            "-u",
            "root",
            "magento",
        ])
        .stdin(reader)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await;
    match out {
        Ok(o) if o.status.success() => json!({ "success": true, "message": "database restored" }),
        Ok(o) => json!({ "success": false, "error": String::from_utf8_lossy(&o.stderr) }),
        Err(e) => json!({ "success": false, "error": e.to_string() }),
    }
}

pub fn delete_backup(s: &Store, filename: &str) -> Value {
    if !valid_filename(filename) {
        return json!({ "success": false, "error": "invalid backup filename" });
    }
    let path = format!("{}/{filename}", backup_dir(&s.name));
    if std::path::Path::new(&path).exists() {
        match std::fs::remove_file(&path) {
            Ok(_) => json!({ "success": true }),
            Err(e) => json!({ "success": false, "error": e.to_string() }),
        }
    } else {
        json!({ "success": false, "error": "backup not found" })
    }
}

/// Keep only the newest `retention` backups; delete the rest.
pub fn prune(s: &Store, retention: usize) -> usize {
    let backups = list_backups(s); // newest first
    let mut removed = 0;
    for b in backups.into_iter().skip(retention.max(1)) {
        if let Some(f) = b["filename"].as_str() {
            let path = format!("{}/{f}", backup_dir(&s.name));
            if std::fs::remove_file(&path).is_ok() {
                removed += 1;
            }
        }
    }
    removed
}

/// Newest backup's age in seconds (None if no backups) — for the scheduler.
pub fn newest_backup_age_secs(s: &Store) -> Option<i64> {
    let backups = list_backups(s);
    let newest = backups.first()?;
    let ts = newest["created_at"].as_str()?;
    let dt = chrono::NaiveDateTime::parse_from_str(ts, "%Y-%m-%dT%H:%M:%S").ok()?;
    Some((chrono::Local::now().naive_local() - dt).num_seconds())
}

#[cfg(test)]
mod tests {
    use super::valid_filename;
    #[test]
    fn filenames() {
        assert!(valid_filename("magento_20260704_010203.sql.gz"));
        assert!(valid_filename("dump.sql"));
        assert!(!valid_filename("../etc/passwd"));
        assert!(!valid_filename("x.txt"));
        assert!(!valid_filename("a/b.sql"));
    }
}
