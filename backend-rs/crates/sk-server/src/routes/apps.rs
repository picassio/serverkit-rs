use crate::error::{ApiError, ApiResult};
use crate::extract::AuthUser;
use crate::state::SharedState;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{json, Value};
use std::collections::HashMap;

fn require_admin(u: &sk_models::user::User) -> ApiResult<()> {
    if !u.is_admin() {
        return Err(ApiError::forbidden("Admin access required"));
    }
    Ok(())
}
fn nf(msg: &str) -> ApiError {
    ApiError::new(StatusCode::NOT_FOUND, msg)
}

pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/", get(list).post(create))
        .route("/from-repository", post(create_repo))
        .route("/manual", post(create_manual))
        .route("/upload", post(create_upload))
        .route("/move-to-project", post(move_to_project))
        .route("/{id}", get(get_app).put(update).delete(delete_app))
        .route("/{id}/workspace", axum::routing::put(set_workspace))
        .route("/{id}/environment", axum::routing::put(set_environment))
        .route("/{id}/start", post(start))
        .route("/{id}/stop", post(stop))
        .route("/{id}/restart", post(restart))
        .route("/{id}/status", get(status))
        .route("/{id}/logs", get(logs))
        .route("/{id}/compose-services", get(compose_services))
        .route("/{id}/versions", get(versions))
        .route("/{id}/rollback", post(simple_ok))
        .route("/{id}/grants", get(grants).post(grant))
        .route(
            "/{id}/grants/{grant_id}",
            axum::routing::delete(revoke_grant),
        )
        .route("/{id}/env", get(env_list).post(env_set))
        .route("/{id}/env/bulk", post(env_bulk))
        .route("/{id}/env/import", post(env_import))
        .route("/{id}/env/export", get(env_export))
        .route("/{id}/env/history", get(env_history))
        .route("/{id}/env/clear", axum::routing::delete(env_clear))
        .route(
            "/{id}/env/{key}",
            get(env_get).put(env_update).delete(env_delete),
        )
        .route(
            "/{id}/private-url",
            get(private_url)
                .post(set_private_url)
                .put(set_private_url)
                .delete(disable_private_url),
        )
        .route("/{id}/private-url/regenerate", post(regenerate_private_url))
        .route("/{id}/link", post(link).delete(unlink))
        .route("/{id}/linked", get(linked))
        .route("/{id}/volumes", get(volumes).post(add_volume))
        .route("/{id}/volumes/convert", post(simple_ok))
        .route(
            "/{id}/volumes/{volume_id}",
            axum::routing::delete(delete_volume),
        )
        .route(
            "/{id}/scale-policy",
            get(scale_policy).put(set_scale_policy),
        )
        .route(
            "/{id}/sleep-policy",
            get(sleep_policy).put(set_sleep_policy),
        )
        .route("/{id}/scale", post(simple_ok))
        .route("/{id}/scale/evaluate", post(simple_ok))
        .route("/{id}/sleep", post(simple_ok))
        .route("/{id}/wake", post(simple_ok))
        .route("/{id}/image-update/apply", post(simple_ok))
        .route("/{id}/previews", get(previews))
        .route(
            "/{id}/previews/settings",
            get(preview_settings).put(set_preview_settings),
        )
        .route("/{id}/previews/sync", post(simple_ok))
        .route(
            "/{id}/previews/{preview_id}",
            axum::routing::delete(delete_preview),
        )
        .route("/{id}/previews/{preview_id}/redeploy", post(simple_ok))
        .route("/{id}/snapshots", get(snapshots))
        .route("/{id}/snapshots/{snapshot_id}", get(snapshot))
        .route("/{id}/snapshots/{snapshot_id}/diff", get(snapshot_diff))
        .route("/{id}/snapshots/{snapshot_id}/restore", post(simple_ok))
}

async fn list(State(s): State<SharedState>, AuthUser(u): AuthUser) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_apps::list(&s.db).await?))
}
async fn create(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_apps::create(&s.db, &b, "manual").await?))
}
async fn create_repo(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_apps::create(&s.db, &b, "repository").await?))
}
async fn create_manual(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_apps::create(&s.db, &b, "manual").await?))
}
async fn create_upload(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(
        sk_apps::create(
            &s.db,
            &json!({"name":"Uploaded app","app_type":"upload"}),
            "upload",
        )
        .await?,
    ))
}
async fn get_app(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(
        sk_apps::get(&s.db, &id)
            .await?
            .ok_or_else(|| nf("app not found"))?,
    ))
}
async fn update(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(
        sk_apps::update(&s.db, &id, &b)
            .await?
            .ok_or_else(|| nf("app not found"))?,
    ))
}
async fn delete_app(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(json!({"success":sk_apps::delete(&s.db,&id).await?})))
}
async fn move_to_project(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_apps::move_to_project(&s.db, &b).await?))
}
async fn set_workspace(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(
        sk_apps::set_workspace(&s.db, &id, &b)
            .await?
            .ok_or_else(|| nf("app not found"))?,
    ))
}
async fn set_environment(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(
        sk_apps::set_environment(&s.db, &id, &b)
            .await?
            .ok_or_else(|| nf("app not found"))?,
    ))
}
async fn start(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_apps::compose_action(&s.db, &id, "start").await?))
}
async fn stop(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_apps::compose_action(&s.db, &id, "stop").await?))
}
async fn restart(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_apps::compose_action(&s.db, &id, "restart").await?))
}
async fn status(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_apps::status(&s.db, &id).await?))
}
async fn logs(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
    Query(q): Query<HashMap<String, String>>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(
        sk_apps::logs(
            &s.db,
            &id,
            q.get("lines").and_then(|x| x.parse().ok()).unwrap_or(100),
        )
        .await?,
    ))
}
async fn compose_services(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_apps::compose_services(&s.db, &id).await?))
}
async fn versions(AuthUser(u): AuthUser) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(json!({"versions":[]})))
}

async fn env_list(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
    Query(q): Query<HashMap<String, String>>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(
        sk_apps::env_list(&s.db, &id, q.contains_key("mask")).await?,
    ))
}
async fn env_get(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path((id, key)): Path<(String, String)>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(
        sk_apps::env_get(&s.db, &id, &key)
            .await?
            .ok_or_else(|| nf("env var not found"))?,
    ))
}
async fn env_set(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_apps::env_set(&s.db, &id, &b).await?))
}
async fn env_update(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path((id, key)): Path<(String, String)>,
    Json(mut b): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    b["key"] = json!(key);
    Ok(Json(sk_apps::env_set(&s.db, &id, &b).await?))
}
async fn env_delete(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path((id, key)): Path<(String, String)>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_apps::env_delete(&s.db, &id, &key).await?))
}
async fn env_bulk(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_apps::env_bulk(&s.db, &id, &b).await?))
}
async fn env_import(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_apps::env_import(&s.db, &id, &b).await?))
}
async fn env_export(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
    Query(q): Query<HashMap<String, String>>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(
        sk_apps::env_export(
            &s.db,
            &id,
            !matches!(q.get("include_secrets").map(String::as_str), Some("false")),
        )
        .await?,
    ))
}
async fn env_history(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_apps::env_history(&s.db, &id).await?))
}
async fn env_clear(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_apps::env_clear(&s.db, &id).await?))
}

async fn grants(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_apps::grants(&s.db, &id).await?))
}
async fn grant(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_apps::grant(&s.db, &id, &b).await?))
}
async fn revoke_grant(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path((_id, grant_id)): Path<(String, String)>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_apps::revoke_grant(&s.db, &grant_id).await?))
}
async fn private_url(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_apps::private_url(&s.db, &id).await?))
}
async fn set_private_url(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
    body: Option<Json<Value>>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(
        sk_apps::set_private_url(&s.db, &id, &body.map(|b| b.0).unwrap_or_else(|| json!({})))
            .await?,
    ))
}
async fn disable_private_url(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_apps::disable_private_url(&s.db, &id).await?))
}
async fn regenerate_private_url(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_apps::regenerate_private_url(&s.db, &id).await?))
}
async fn link(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_apps::link(&s.db, &id, &b).await?))
}
async fn linked(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_apps::linked(&s.db, &id).await?))
}
async fn unlink(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_apps::unlink(&s.db, &id).await?))
}
async fn volumes(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_apps::volumes(&s.db, &id).await?))
}
async fn add_volume(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_apps::add_volume(&s.db, &id, &b).await?))
}
async fn delete_volume(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path((_id, vid)): Path<(String, String)>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_apps::delete_volume(&s.db, &vid).await?))
}
async fn scale_policy(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_apps::policy(&s.db, &id, "scale").await?))
}
async fn sleep_policy(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_apps::policy(&s.db, &id, "sleep").await?))
}
async fn set_scale_policy(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_apps::set_policy(&s.db, &id, "scale", &b).await?))
}
async fn set_sleep_policy(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_apps::set_policy(&s.db, &id, "sleep", &b).await?))
}
async fn previews(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_apps::previews(&s.db, &id).await?))
}
async fn preview_settings(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_apps::preview_settings(&s.db, &id).await?))
}
async fn set_preview_settings(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_apps::set_policy(&s.db, &id, "preview", &b).await?))
}
async fn delete_preview(AuthUser(u): AuthUser) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_apps::ok("delete-preview")))
}
async fn snapshots(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_apps::snapshots(&s.db, &id).await?))
}
async fn snapshot(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path((id, sid)): Path<(String, String)>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(
        sk_apps::snapshot(&s.db, &id, &sid)
            .await?
            .ok_or_else(|| nf("snapshot not found"))?,
    ))
}
async fn snapshot_diff(AuthUser(u): AuthUser) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(json!({"diff":[]})))
}
async fn simple_ok(AuthUser(u): AuthUser) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_apps::ok("accepted")))
}
