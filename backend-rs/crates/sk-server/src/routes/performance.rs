use crate::error::{ApiError, ApiResult};
use crate::extract::AuthUser;
use crate::state::SharedState;
use axum::extract::{Path, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{json, Value};
use std::path::{Path as FsPath, PathBuf};
use tokio::fs;

fn internal(e: anyhow::Error) -> ApiError {
    e.into()
}
fn require_admin(u: &sk_models::user::User) -> ApiResult<()> {
    if !u.is_admin() {
        Err(ApiError::forbidden("Admin access required"))
    } else {
        Ok(())
    }
}
fn cache_root() -> PathBuf {
    std::env::var("SK_CACHE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let data = std::env::var("SK_DATA_DIR").unwrap_or_else(|_| "data".into());
            PathBuf::from(data).join("cache")
        })
}
async fn dir_size(path: &FsPath) -> (u64, u64) {
    let mut bytes = 0;
    let mut files = 0;
    let mut stack = vec![path.to_path_buf()];
    while let Some(p) = stack.pop() {
        let Ok(mut rd) = fs::read_dir(&p).await else {
            continue;
        };
        while let Ok(Some(e)) = rd.next_entry().await {
            let Ok(m) = e.metadata().await else { continue };
            if m.is_dir() {
                stack.push(e.path())
            } else {
                bytes += m.len();
                files += 1
            }
        }
    }
    (bytes, files)
}
async fn managed_caches() -> Value {
    let root = cache_root();
    let _ = fs::create_dir_all(&root).await;
    let (bytes, files) = dir_size(&root).await;
    let mem = std::fs::read_to_string("/proc/meminfo").unwrap_or_default();
    let cached_kb = mem
        .lines()
        .find_map(|l| {
            l.strip_prefix("Cached:")
                .and_then(|x| x.split_whitespace().next())
                .and_then(|x| x.parse::<u64>().ok())
        })
        .unwrap_or(0);
    json!({"success":true,"cache_dir":root,"managed":{"bytes":bytes,"files":files},"os":{"cached_kb":cached_kb},"caches":[{"id":"serverkit-managed","name":"ServerKit managed cache","path":root,"bytes":bytes,"files":files,"flushable":true}]})
}
async fn flush_managed() -> Value {
    let root = cache_root();
    let _ = fs::create_dir_all(&root).await;
    let (before_bytes, before_files) = dir_size(&root).await;
    let mut removed = 0u64;
    if let Ok(mut rd) = fs::read_dir(&root).await {
        while let Ok(Some(e)) = rd.next_entry().await {
            let p = e.path();
            let res = if p.is_dir() {
                fs::remove_dir_all(&p).await
            } else {
                fs::remove_file(&p).await
            };
            if res.is_ok() {
                removed += 1;
            }
        }
    }
    json!({"success":true,"flushed":"serverkit-managed","removed_entries":removed,"before":{"bytes":before_bytes,"files":before_files}})
}
pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/cache/stats", get(cache_stats))
        .route("/cache/flush", post(cache_flush))
        .route("/jobs", get(jobs))
        .route("/jobs/stats", get(job_stats))
        .route("/jobs/cleanup", post(cleanup_jobs))
        .route("/jobs/{id}", get(job_detail))
}
async fn cache_stats(AuthUser(u): AuthUser) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(managed_caches().await))
}
async fn cache_flush(AuthUser(u): AuthUser) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(flush_managed().await))
}
async fn jobs(State(s): State<SharedState>, AuthUser(u): AuthUser) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(
        sk_jobs::list_jobs(&s.db, &json!({"limit":100}))
            .await
            .map_err(internal)?,
    ))
}
async fn job_detail(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    let Some(job) = sk_jobs::get_job(&s.db, &id).await.map_err(internal)? else {
        return Err(ApiError::not_found("Job not found"));
    };
    Ok(Json(json!({"job":job})))
}
async fn job_stats(State(s): State<SharedState>, AuthUser(u): AuthUser) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(
        json!({"success":true,"stats":sk_jobs::job_stats(&s.db).await.map_err(internal)?}),
    ))
}
async fn cleanup_jobs(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    let r=sqlx::query("DELETE FROM sk_jobs WHERE status IN ('completed','failed','cancelled') AND created_at < datetime('now','-7 days')").execute(&s.db).await.map_err(ApiError::from)?;
    Ok(Json(json!({"success":true,"deleted":r.rows_affected()})))
}
