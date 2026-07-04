//! Docker-hosted MySQL/MariaDB — the primary DB mode for the Magento fork
//! (provisioner architecture: data services live in containers, nginx+PHP
//! stay native). Ports `docker_mysql_*` from `database_service.py`.

use serde_json::{json, Value};
use std::process::Stdio;
use tokio::process::Command;

const MYSQL_IMAGES: &[&str] = &["mysql", "mariadb", "percona"];
const SYSTEM_DBS: &[&str] = &["information_schema", "mysql", "performance_schema", "sys"];

/// Container names/ids from `docker ps` are [a-zA-Z0-9][a-zA-Z0-9_.-]* — reject anything else.
fn valid_container(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .next()
            .map(|c| c.is_ascii_alphanumeric())
            .unwrap_or(false)
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '-')
}

fn valid_user(user: &str) -> bool {
    !user.is_empty()
        && user
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$' || c == '.')
}

struct Out {
    ok: bool,
    stdout: String,
    stderr: String,
}

/// MariaDB 11+ images ship only the `mariadb` client (no `mysql` symlink) —
/// upstream Flask breaks on those. Try `mysql`, fall back to `mariadb`.
async fn docker_mysql(
    container: &str,
    user: &str,
    password: Option<&str>,
    extra: &[&str],
    stdin: Option<&str>,
    timeout: u64,
) -> Out {
    let first = docker_mysql_with(container, "mysql", user, password, extra, stdin, timeout).await;
    // docker ≥29 emits the OCI exec error on *stdout*, older versions on stderr
    if !first.ok
        && (first.stderr.contains("executable file not found")
            || first.stdout.contains("executable file not found"))
    {
        return docker_mysql_with(container, "mariadb", user, password, extra, stdin, timeout)
            .await;
    }
    first
}

async fn docker_mysql_with(
    container: &str,
    client: &str,
    user: &str,
    password: Option<&str>,
    extra: &[&str],
    stdin: Option<&str>,
    timeout: u64,
) -> Out {
    let mut cmd = Command::new("docker");
    cmd.arg("exec");
    if stdin.is_some() {
        cmd.arg("-i");
    }
    // MYSQL_PWD env inside the container — never on the client argv
    let env_arg;
    if let Some(pw) = password {
        env_arg = format!("MYSQL_PWD={pw}");
        cmd.args(["-e", &env_arg]);
    }
    cmd.args([container, client, "-u", user]);
    cmd.args(extra);
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

/// `list_docker_mysql_containers` — running containers with MySQL-ish images.
pub async fn list_containers() -> Vec<Value> {
    let out = Command::new("docker")
        .args(["ps", "--format", "{{json .}}"])
        .output()
        .await;
    let Ok(out) = out else { return Vec::new() };
    if !out.status.success() {
        return Vec::new();
    }
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .filter(|c| {
            let image = c["Image"].as_str().unwrap_or("").to_lowercase();
            MYSQL_IMAGES.iter().any(|img| image.contains(img))
        })
        .map(|c| {
            json!({
                "id": c["ID"],
                "name": c["Names"],
                "image": c["Image"],
                "status": c["Status"],
                "ports": c["Ports"],
                "type": "mysql",
            })
        })
        .collect()
}

/// `docker_mysql_list_databases`
pub async fn list_databases(container: &str, user: &str, password: Option<&str>) -> Vec<Value> {
    if !valid_container(container) || !valid_user(user) {
        return Vec::new();
    }
    let r = docker_mysql(
        container,
        user,
        password,
        &["-e", "SHOW DATABASES;"],
        None,
        30,
    )
    .await;
    if !r.ok {
        return Vec::new();
    }
    r.stdout
        .trim()
        .lines()
        .skip(1)
        .filter_map(|line| {
            let name = line.trim();
            (!name.is_empty() && !SYSTEM_DBS.contains(&name))
                .then(|| json!({ "name": name, "type": "docker_mysql", "container": container }))
        })
        .collect()
}

/// `docker_mysql_get_tables` — `connected` distinguishes an empty DB from a
/// broken container/auth (upstream bug fix preserved).
pub async fn tables(container: &str, database: &str, user: &str, password: Option<&str>) -> Value {
    if !valid_container(container) || !super::validate_identifier(database, 64) || !valid_user(user)
    {
        return json!({ "connected": false, "tables": [], "error": "Invalid identifiers" });
    }
    let r = docker_mysql(
        container,
        user,
        password,
        &["-D", database, "-e", "SHOW TABLES;"],
        None,
        30,
    )
    .await;
    if !r.ok {
        return json!({
            "connected": false,
            "tables": [],
            "error": if r.stderr.trim().is_empty() {
                "Could not connect to the database container".to_string()
            } else {
                r.stderr.trim().to_string()
            },
        });
    }

    let table_names: Vec<String> = r
        .stdout
        .trim()
        .lines()
        .skip(1)
        .map(|l| l.trim().to_string())
        .filter(|t| !t.is_empty() && super::validate_identifier(t, 64))
        .collect();

    let mut tables = Vec::new();
    for table in table_names {
        let count_q = format!("SELECT COUNT(*) FROM `{table}`;");
        let count = docker_mysql(
            container,
            user,
            password,
            &["-D", database, "-e", &count_q],
            None,
            30,
        )
        .await;
        let rows: i64 = count
            .stdout
            .trim()
            .lines()
            .nth(1)
            .and_then(|l| l.trim().parse().ok())
            .unwrap_or(0);
        tables.push(json!({ "name": table, "rows": rows }));
    }
    json!({ "connected": true, "tables": tables, "error": Value::Null })
}

/// `docker_mysql_execute_query` — SQL console against a container.
#[allow(clippy::too_many_arguments)]
pub async fn execute_query(
    container: &str,
    database: &str,
    query: &str,
    user: &str,
    password: Option<&str>,
    readonly: bool,
    timeout: u64,
    max_rows: usize,
) -> Value {
    if readonly && !super::is_readonly_query(query) {
        return json!({
            "success": false,
            "error": "Only SELECT, SHOW, DESCRIBE, and EXPLAIN queries are allowed in readonly mode"
        });
    }
    if !valid_container(container) || !super::validate_identifier(database, 64) || !valid_user(user)
    {
        return json!({ "success": false, "error": "Invalid identifiers" });
    }

    let start = std::time::Instant::now();
    let r = docker_mysql(
        container,
        user,
        password,
        &["-D", database, "-e", query, "--batch"],
        None,
        timeout,
    )
    .await;
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

    let columns: Vec<String> = lines[0].split('\t').map(str::to_string).collect();
    let data_lines = &lines[1..];
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn container_validation() {
        assert!(valid_container("magento-mysql-1"));
        assert!(valid_container("3da9d1ae9d80"));
        assert!(!valid_container("-bad"));
        assert!(!valid_container("a;b"));
        assert!(!valid_container(""));
    }
}
