//! sk-db — MySQL/MariaDB management, ported from the MySQL half of
//! `app/services/database_service.py`. PostgreSQL/SQLite/docker-DB explorer
//! land in a later slice; `status()` already reports both engines.
//!
//! Passwords never hit argv: root passwords travel via the `MYSQL_PWD` env
//! var and user passwords are hex-encoded through a PREPARE statement,
//! exactly like the Flask oracle.

pub mod docker;

use serde_json::{json, Value};
use std::process::Stdio;
use tokio::process::Command;

const SYSTEM_DBS: &[&str] = &["information_schema", "mysql", "performance_schema", "sys"];
const SYSTEM_USERS: &[&str] = &[
    "root",
    "mysql.sys",
    "mysql.session",
    "mysql.infoschema",
    "debian-sys-maint",
];
const READONLY_COMMANDS: &[&str] = &["SELECT", "SHOW", "DESCRIBE", "DESC", "EXPLAIN"];

fn backup_dir() -> String {
    std::env::var("SK_DB_BACKUP_DIR").unwrap_or_else(|_| "/var/backups/serverkit/databases".into())
}

/// `_validate_identifier` — alphanumeric + `_$`, max 64 (128 for collations).
pub fn validate_identifier(name: &str, max_length: usize) -> bool {
    !name.is_empty()
        && name.len() <= max_length
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$')
}

struct Out {
    ok: bool,
    stdout: String,
    stderr: String,
}

async fn mysql_run(
    args: &[&str],
    stdin: Option<&str>,
    root_password: Option<&str>,
    timeout: u64,
) -> Out {
    let mut cmd = Command::new("mysql");
    cmd.args(args);
    if let Some(pw) = root_password {
        cmd.env("MYSQL_PWD", pw);
    }
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    if stdin.is_some() {
        cmd.stdin(Stdio::piped());
    }

    let run = async {
        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                return Out {
                    ok: false,
                    stdout: String::new(),
                    stderr: e.to_string(),
                }
            }
        };
        if let (Some(input), Some(mut pipe)) = (stdin, child.stdin.take()) {
            use tokio::io::AsyncWriteExt;
            let _ = pipe.write_all(input.as_bytes()).await;
            drop(pipe);
        }
        match child.wait_with_output().await {
            Ok(out) => Out {
                ok: out.status.success(),
                stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
                stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
            },
            Err(e) => Out {
                ok: false,
                stdout: String::new(),
                stderr: e.to_string(),
            },
        }
    };
    match tokio::time::timeout(std::time::Duration::from_secs(timeout), run).await {
        Ok(o) => o,
        Err(_) => Out {
            ok: false,
            stdout: String::new(),
            stderr: "Query timed out".into(),
        },
    }
}

/// `DatabaseService.mysql_execute`
async fn mysql_execute(query: &str, database: Option<&str>, root_password: Option<&str>) -> Value {
    let mut args = vec!["-u", "root"];
    if let Some(db) = database {
        args.extend(["-D", db]);
    }
    args.extend(["-e", query]);
    let r = mysql_run(&args, None, root_password, 30).await;
    json!({
        "success": r.ok,
        "output": r.stdout,
        "error": if r.ok { Value::Null } else { json!(r.stderr) },
    })
}

async fn cmd_ok(cmd: &str, args: &[&str]) -> bool {
    Command::new(cmd)
        .args(args)
        .output()
        .await
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// `DatabaseService.get_status`
pub async fn status() -> Value {
    let mysql_installed = cmd_ok("mysql", &["--version"]).await;
    let mysql_running = cmd_ok("systemctl", &["is-active", "mysql"]).await
        || cmd_ok("systemctl", &["is-active", "mariadb"]).await;
    let pg_installed = cmd_ok("psql", &["--version"]).await;
    let pg_running = cmd_ok("systemctl", &["is-active", "postgresql"]).await;
    json!({
        "mysql": { "installed": mysql_installed, "running": mysql_running },
        "postgresql": { "installed": pg_installed, "running": pg_running },
    })
}

/// `mysql_list_databases` — with per-DB size from information_schema.
pub async fn list_databases(root_password: Option<&str>) -> Vec<Value> {
    let result = mysql_execute("SHOW DATABASES;", None, root_password).await;
    if !result["success"].as_bool().unwrap_or(false) {
        return Vec::new();
    }
    let output = result["output"].as_str().unwrap_or("");
    let mut databases = Vec::new();
    for line in output.trim().lines().skip(1) {
        let name = line.trim();
        if name.is_empty() || SYSTEM_DBS.contains(&name) {
            continue;
        }
        // hex-encoded literal, injection-safe (mirrors _mysql_execute_parameterized)
        let hexed = hex::encode(name.as_bytes());
        let size_q = format!(
            "SELECT SUM(data_length + index_length) FROM information_schema.tables WHERE table_schema = 0x{hexed};"
        );
        let size_r = mysql_run(
            &["-u", "root", "--batch", "-N", "-e", &size_q],
            None,
            root_password,
            30,
        )
        .await;
        let size: i64 = size_r
            .stdout
            .trim()
            .lines()
            .next()
            .and_then(|l| l.trim().parse().ok())
            .unwrap_or(0);
        databases.push(json!({ "name": name, "size": size, "type": "mysql" }));
    }
    databases
}

/// `mysql_create_database`
pub async fn create_database(
    name: &str,
    charset: &str,
    collation: &str,
    root_password: Option<&str>,
) -> Value {
    if !validate_identifier(name, 64) {
        return json!({ "success": false, "error": "Invalid identifier: only alphanumeric characters and underscores allowed" });
    }
    if !validate_identifier(charset, 64) {
        return json!({ "success": false, "error": "Invalid charset identifier" });
    }
    if !validate_identifier(collation, 128) {
        return json!({ "success": false, "error": "Invalid collation identifier" });
    }
    mysql_execute(
        &format!(
            "CREATE DATABASE IF NOT EXISTS `{name}` CHARACTER SET {charset} COLLATE {collation};"
        ),
        None,
        root_password,
    )
    .await
}

/// `mysql_drop_database`
pub async fn drop_database(name: &str, root_password: Option<&str>) -> Value {
    if !validate_identifier(name, 64) {
        return json!({ "success": false, "error": "Invalid identifier: only alphanumeric characters and underscores allowed" });
    }
    mysql_execute(
        &format!("DROP DATABASE IF EXISTS `{name}`;"),
        None,
        root_password,
    )
    .await
}

/// `mysql_get_tables` — with row counts.
pub async fn tables(database: &str, root_password: Option<&str>) -> Vec<Value> {
    if !validate_identifier(database, 64) {
        return Vec::new();
    }
    let result = mysql_execute("SHOW TABLES;", Some(database), root_password).await;
    if !result["success"].as_bool().unwrap_or(false) {
        return Vec::new();
    }
    let output = result["output"].as_str().unwrap_or("").to_string();
    let mut tables = Vec::new();
    for line in output.trim().lines().skip(1) {
        let table = line.trim();
        if table.is_empty() || !validate_identifier(table, 64) {
            continue;
        }
        let count = mysql_execute(
            &format!("SELECT COUNT(*) FROM `{table}`;"),
            Some(database),
            root_password,
        )
        .await;
        let rows: i64 = count["output"]
            .as_str()
            .unwrap_or("")
            .trim()
            .lines()
            .nth(1)
            .and_then(|l| l.trim().parse().ok())
            .unwrap_or(0);
        tables.push(json!({ "name": table, "rows": rows }));
    }
    tables
}

/// `mysql_list_users`
pub async fn list_users(root_password: Option<&str>) -> Vec<Value> {
    let result = mysql_execute("SELECT User, Host FROM mysql.user;", None, root_password).await;
    if !result["success"].as_bool().unwrap_or(false) {
        return Vec::new();
    }
    result["output"]
        .as_str()
        .unwrap_or("")
        .trim()
        .lines()
        .skip(1)
        .filter_map(|line| {
            let mut parts = line.trim().split('\t');
            let user = parts.next()?;
            let host = parts.next()?;
            (!SYSTEM_USERS.contains(&user)).then(|| json!({ "user": user, "host": host }))
        })
        .collect()
}

/// `mysql_create_user` — hex-encoded password via PREPARE (identical SQL to Flask).
pub async fn create_user(
    username: &str,
    password: &str,
    host: &str,
    root_password: Option<&str>,
) -> Value {
    if !validate_identifier(username, 64) {
        return json!({ "success": false, "error": "Invalid identifier: only alphanumeric characters and underscores allowed" });
    }
    if !validate_identifier(host, 64) && host != "localhost" && host != "%" {
        return json!({ "success": false, "error": "Invalid host identifier" });
    }
    let hex_pw = hex::encode(password.as_bytes());
    let stmt = format!(
        "SET @pw = UNHEX('{hex_pw}');\n\
         SET @pw = CAST(@pw AS CHAR);\n\
         SET @sql = CONCAT('CREATE USER IF NOT EXISTS ''{username}''@''{host}'' IDENTIFIED BY ', QUOTE(@pw));\n\
         PREPARE stmt FROM @sql;\n\
         EXECUTE stmt;\n\
         DEALLOCATE PREPARE stmt;\n"
    );
    let r = mysql_run(&["-u", "root"], Some(&stmt), root_password, 30).await;
    json!({
        "success": r.ok,
        "output": r.stdout,
        "error": if r.ok { Value::Null } else { json!(r.stderr) },
    })
}

/// `mysql_drop_user`
pub async fn drop_user(username: &str, host: &str, root_password: Option<&str>) -> Value {
    if !validate_identifier(username, 64) {
        return json!({ "success": false, "error": "Invalid identifier: only alphanumeric characters and underscores allowed" });
    }
    if !validate_identifier(host, 64) && host != "localhost" && host != "%" {
        return json!({ "success": false, "error": "Invalid host identifier" });
    }
    mysql_execute(
        &format!("DROP USER IF EXISTS '{username}'@'{host}';"),
        None,
        root_password,
    )
    .await
}

fn valid_privileges(privileges: &str) -> bool {
    // e.g. "ALL", "SELECT, INSERT, UPDATE" — letters, commas, spaces only
    !privileges.is_empty()
        && privileges
            .chars()
            .all(|c| c.is_ascii_alphabetic() || c == ',' || c == ' ')
}

/// `mysql_grant_privileges`
pub async fn grant(
    username: &str,
    database: &str,
    privileges: &str,
    host: &str,
    root_password: Option<&str>,
) -> Value {
    if !validate_identifier(username, 64) || !validate_identifier(database, 64) {
        return json!({ "success": false, "error": "Invalid identifier: only alphanumeric characters and underscores allowed" });
    }
    if (!validate_identifier(host, 64) && host != "localhost" && host != "%")
        || !valid_privileges(privileges)
    {
        return json!({ "success": false, "error": "Invalid host or privileges" });
    }
    mysql_execute(
        &format!(
            "GRANT {privileges} ON `{database}`.* TO '{username}'@'{host}'; FLUSH PRIVILEGES;"
        ),
        None,
        root_password,
    )
    .await
}

/// `mysql_revoke_privileges`
pub async fn revoke(
    username: &str,
    database: &str,
    privileges: &str,
    host: &str,
    root_password: Option<&str>,
) -> Value {
    if !validate_identifier(username, 64) || !validate_identifier(database, 64) {
        return json!({ "success": false, "error": "Invalid identifier: only alphanumeric characters and underscores allowed" });
    }
    if (!validate_identifier(host, 64) && host != "localhost" && host != "%")
        || !valid_privileges(privileges)
    {
        return json!({ "success": false, "error": "Invalid host or privileges" });
    }
    mysql_execute(
        &format!(
            "REVOKE {privileges} ON `{database}`.* FROM '{username}'@'{host}'; FLUSH PRIVILEGES;"
        ),
        None,
        root_password,
    )
    .await
}

/// `mysql_get_user_privileges`
pub async fn user_privileges(
    username: &str,
    host: &str,
    root_password: Option<&str>,
) -> Vec<Value> {
    if !validate_identifier(username, 64) {
        return Vec::new();
    }
    if !validate_identifier(host, 64) && host != "localhost" && host != "%" {
        return Vec::new();
    }
    let result = mysql_execute(
        &format!("SHOW GRANTS FOR '{username}'@'{host}';"),
        None,
        root_password,
    )
    .await;
    if !result["success"].as_bool().unwrap_or(false) {
        return Vec::new();
    }
    result["output"]
        .as_str()
        .unwrap_or("")
        .trim()
        .lines()
        .skip(1)
        .map(|l| json!(l.trim()))
        .collect()
}

/// `_is_readonly_query`
pub fn is_readonly_query(query: &str) -> bool {
    query
        .trim()
        .to_uppercase()
        .split_whitespace()
        .next()
        .map(|w| READONLY_COMMANDS.contains(&w))
        .unwrap_or(false)
}

/// `mysql_execute_query` — structured results for the SQL console.
pub async fn execute_query(
    database: &str,
    query: &str,
    readonly: bool,
    root_password: Option<&str>,
    timeout: u64,
    max_rows: usize,
) -> Value {
    if readonly && !is_readonly_query(query) {
        return json!({
            "success": false,
            "error": "Only SELECT, SHOW, DESCRIBE, and EXPLAIN queries are allowed in readonly mode"
        });
    }
    if !validate_identifier(database, 64) {
        return json!({ "success": false, "error": "Invalid database identifier" });
    }

    let start = std::time::Instant::now();
    let upper = query.trim().to_uppercase();
    let headerless =
        upper.starts_with("SHOW") || upper.starts_with("DESCRIBE") || upper.starts_with("DESC");

    let mut args = vec!["-u", "root", "-D", database, "-e", query, "--batch"];
    if headerless {
        args.push("-N");
    }
    let r = mysql_run(&args, None, root_password, timeout).await;
    let execution_time = start.elapsed().as_secs_f64();

    if !r.ok {
        return json!({
            "success": false,
            "error": if r.stderr.trim().is_empty() { "Query execution failed".to_string() } else { r.stderr.trim().to_string() }
        });
    }

    let lines: Vec<&str> = if r.stdout.trim().is_empty() {
        Vec::new()
    } else {
        r.stdout.trim().lines().collect()
    };

    if lines.is_empty() {
        return json!({
            "success": true, "columns": [], "rows": [], "row_count": 0,
            "execution_time": execution_time, "truncated": false
        });
    }

    let (columns, data_lines): (Vec<String>, Vec<&str>) = if headerless {
        let n = lines[0].split('\t').count();
        (
            (0..n).map(|i| format!("Column_{i}")).collect(),
            lines.clone(),
        )
    } else {
        (
            lines[0].split('\t').map(str::to_string).collect(),
            lines[1..].to_vec(),
        )
    };

    let rows: Vec<Vec<Value>> = data_lines
        .iter()
        .take(max_rows)
        .map(|line| {
            line.split('\t')
                .map(|v| if v == "NULL" { Value::Null } else { json!(v) })
                .collect()
        })
        .collect();

    json!({
        "success": true,
        "columns": columns,
        "row_count": rows.len(),
        "total_rows": data_lines.len(),
        "rows": rows,
        "execution_time": (execution_time * 10000.0).round() / 10000.0,
        "truncated": data_lines.len() > max_rows,
    })
}

/// `mysql_backup` — mysqldump | gzip → BACKUP_DIR.
pub async fn backup(database: &str, root_password: Option<&str>) -> Value {
    if !validate_identifier(database, 64) {
        return json!({ "success": false, "error": "Invalid identifier" });
    }
    let dir = backup_dir();
    if let Err(e) = std::fs::create_dir_all(&dir) {
        return json!({ "success": false, "error": e.to_string() });
    }
    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
    let output_path = format!("{dir}/mysql_{database}_{timestamp}.sql.gz");

    let mut dump = Command::new("mysqldump");
    dump.args(["-u", "root", database]);
    if let Some(pw) = root_password {
        dump.env("MYSQL_PWD", pw);
    }
    dump.stdout(Stdio::piped()).stderr(Stdio::piped());

    let mut dump_child = match dump.spawn() {
        Ok(c) => c,
        Err(e) => return json!({ "success": false, "error": e.to_string() }),
    };
    let dump_out: Stdio = match dump_child.stdout.take().map(|s| s.try_into()) {
        Some(Ok(s)) => s,
        _ => return json!({ "success": false, "error": "failed to pipe mysqldump" }),
    };

    let outfile = match std::fs::File::create(&output_path) {
        Ok(f) => f,
        Err(e) => return json!({ "success": false, "error": e.to_string() }),
    };
    let gzip = Command::new("gzip")
        .arg("-c")
        .stdin(dump_out)
        .stdout(Stdio::from(outfile))
        .status();

    let (dump_status, gzip_status) = tokio::join!(dump_child.wait(), gzip);
    let dump_ok = dump_status.map(|s| s.success()).unwrap_or(false);
    let gzip_ok = gzip_status.map(|s| s.success()).unwrap_or(false);

    if dump_ok && gzip_ok {
        let size = std::fs::metadata(&output_path)
            .map(|m| m.len())
            .unwrap_or(0);
        json!({ "success": true, "path": output_path, "size": size })
    } else {
        let mut stderr = String::new();
        if let Some(mut e) = dump_child.stderr.take() {
            use tokio::io::AsyncReadExt;
            let _ = e.read_to_string(&mut stderr).await;
        }
        let _ = std::fs::remove_file(&output_path);
        json!({ "success": false, "error": if stderr.is_empty() { "Backup failed".into() } else { stderr } })
    }
}

/// `mysql_restore` — gunzip -c | mysql (or plain file stdin).
pub async fn restore(database: &str, backup_path: &str, root_password: Option<&str>) -> Value {
    if !validate_identifier(database, 64) {
        return json!({ "success": false, "error": "Invalid identifier" });
    }
    if !std::path::Path::new(backup_path).exists() {
        return json!({ "success": false, "error": "Backup file not found" });
    }
    // Restrict restores to the managed backup dir (defense in depth beyond Flask)
    let canonical = std::fs::canonicalize(backup_path)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default();
    if !canonical.starts_with(&backup_dir()) {
        return json!({ "success": false, "error": "Access denied: backup path outside backup directory" });
    }

    let sql: Stdio = if backup_path.ends_with(".gz") {
        let mut gunzip = match Command::new("gunzip")
            .args(["-c", backup_path])
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
        match std::fs::File::open(backup_path) {
            Ok(f) => Stdio::from(f),
            Err(e) => return json!({ "success": false, "error": e.to_string() }),
        }
    };

    let mut cmd = Command::new("mysql");
    cmd.args(["-u", "root", database])
        .stdin(sql)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(pw) = root_password {
        cmd.env("MYSQL_PWD", pw);
    }
    match cmd.output().await {
        Ok(out) if out.status.success() => {
            json!({ "success": true, "message": "Database restored successfully" })
        }
        Ok(out) => json!({ "success": false, "error": String::from_utf8_lossy(&out.stderr) }),
        Err(e) => json!({ "success": false, "error": e.to_string() }),
    }
}

/// `list_backups`
pub fn list_backups(db_type: Option<&str>) -> Vec<Value> {
    let dir = backup_dir();
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut backups: Vec<Value> = entries
        .flatten()
        .filter_map(|e| {
            let filename = e.file_name().to_string_lossy().into_owned();
            if !filename.ends_with(".sql") && !filename.ends_with(".sql.gz") {
                return None;
            }
            let backup_type = if filename.starts_with("mysql_") {
                "mysql"
            } else {
                "postgresql"
            };
            if let Some(t) = db_type {
                if t != backup_type {
                    return None;
                }
            }
            let stem = filename
                .trim_end_matches(".sql.gz")
                .trim_end_matches(".sql");
            let parts: Vec<&str> = stem.split('_').collect();
            let db_name = if parts.len() > 3 {
                parts[1..parts.len() - 2].join("_")
            } else {
                parts.get(1).unwrap_or(&"unknown").to_string()
            };
            let meta = e.metadata().ok()?;
            let created = meta
                .created()
                .or(meta.modified())
                .ok()
                .map(|t| {
                    chrono::DateTime::<chrono::Local>::from(t)
                        .naive_local()
                        .format("%Y-%m-%dT%H:%M:%S%.6f")
                        .to_string()
                })
                .unwrap_or_default();
            Some(json!({
                "filename": filename,
                "path": format!("{dir}/{}", e.file_name().to_string_lossy()),
                "type": backup_type,
                "database": db_name,
                "size": meta.len(),
                "created_at": created,
            }))
        })
        .collect();
    backups.sort_by(|a, b| {
        b["created_at"]
            .as_str()
            .unwrap_or("")
            .cmp(a["created_at"].as_str().unwrap_or(""))
    });
    backups
}

/// `delete_backup` — filename only, no traversal.
pub fn delete_backup(filename: &str) -> Value {
    if filename.contains('/') || filename.contains("..") {
        return json!({ "success": false, "error": "Invalid filename" });
    }
    let path = format!("{}/{filename}", backup_dir());
    if std::path::Path::new(&path).exists() {
        match std::fs::remove_file(&path) {
            Ok(_) => json!({ "success": true }),
            Err(e) => json!({ "success": false, "error": e.to_string() }),
        }
    } else {
        json!({ "success": false, "error": "Backup not found" })
    }
}

/// `generate_password`
pub fn generate_password(length: usize) -> String {
    use rand::Rng;
    const ALPHABET: &[u8] =
        b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789!@#$%^&*";
    let mut rng = rand::thread_rng();
    (0..length)
        .map(|_| ALPHABET[rng.gen_range(0..ALPHABET.len())] as char)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifiers() {
        assert!(validate_identifier("magento_db", 64));
        assert!(!validate_identifier("bad-name", 64));
        assert!(!validate_identifier("a; DROP TABLE", 64));
        assert!(!validate_identifier("", 64));
    }

    #[test]
    fn readonly_gate() {
        assert!(is_readonly_query("  select * from x"));
        assert!(is_readonly_query("SHOW TABLES"));
        assert!(!is_readonly_query("DROP TABLE x"));
        assert!(!is_readonly_query("update x set y=1"));
    }
}
