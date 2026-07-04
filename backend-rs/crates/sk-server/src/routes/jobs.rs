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

fn err(status: StatusCode, message: impl Into<String>) -> ApiError {
    ApiError::new(status, message.into())
}

fn qs_value(q: HashMap<String, String>) -> Value {
    Value::Object(q.into_iter().map(|(k, v)| (k, Value::String(v))).collect())
}

pub fn jobs_router() -> Router<SharedState> {
    Router::new()
        .route("/", get(list_jobs))
        .route("/stats", get(job_stats))
        .route("/kinds", get(job_kinds))
        .route("/scheduled", get(scheduled_jobs))
        .route("/scheduled/{id}/run", post(run_scheduled))
        .route("/scheduled/{id}/enabled", post(set_scheduled_enabled))
        .route("/{id}", get(get_job))
        .route("/{id}/cancel", post(cancel_job))
        .route("/{id}/retry", post(retry_job))
}

async fn list_jobs(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Query(q): Query<HashMap<String, String>>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_jobs::list_jobs(&s.db, &qs_value(q)).await?))
}

async fn get_job(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    let job = sk_jobs::get_job(&s.db, &id)
        .await?
        .ok_or_else(|| err(StatusCode::NOT_FOUND, "job not found"))?;
    Ok(Json(job))
}

async fn cancel_job(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    let job = sk_jobs::cancel_job(&s.db, &id)
        .await?
        .ok_or_else(|| err(StatusCode::NOT_FOUND, "job not found"))?;
    Ok(Json(json!({ "success": true, "job": job })))
}

async fn retry_job(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    let job = sk_jobs::retry_job(&s.db, &id)
        .await?
        .ok_or_else(|| err(StatusCode::NOT_FOUND, "job not found"))?;
    Ok(Json(json!({ "success": true, "job": job })))
}

async fn job_stats(State(s): State<SharedState>, AuthUser(u): AuthUser) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_jobs::job_stats(&s.db).await?))
}

async fn job_kinds(State(s): State<SharedState>, AuthUser(u): AuthUser) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_jobs::job_kinds(&s.db).await?))
}

async fn scheduled_jobs(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_jobs::scheduled_jobs(&s.db).await?))
}

async fn run_scheduled(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    let job = sk_jobs::run_scheduled(&s.db, &id)
        .await?
        .ok_or_else(|| err(StatusCode::NOT_FOUND, "scheduled job not found"))?;
    Ok(Json(json!({ "success": true, "job": job })))
}

async fn set_scheduled_enabled(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    let enabled = body.get("enabled").and_then(Value::as_bool).unwrap_or(true);
    let scheduled = sk_jobs::set_scheduled_enabled(&s.db, &id, enabled)
        .await?
        .ok_or_else(|| err(StatusCode::NOT_FOUND, "scheduled job not found"))?;
    Ok(Json(json!({ "success": true, "scheduled": scheduled })))
}

pub fn queue_router() -> Router<SharedState> {
    Router::new()
        .route("/stats", get(global_queue_stats))
        .route("/groups", get(queue_groups).post(create_group))
        .route(
            "/groups/{group}",
            get(get_group).patch(update_group).delete(delete_group),
        )
        .route("/groups/{group}/stats", get(group_stats))
        .route("/groups/{group}/queues", get(queues).post(create_queue))
        .route(
            "/groups/{group}/queues/{queue}",
            get(get_queue).patch(update_queue).delete(delete_queue),
        )
        .route("/groups/{group}/queues/{queue}/stats", get(queue_stats))
        .route(
            "/groups/{group}/queues/{queue}/messages",
            get(messages).post(send_message),
        )
        .route(
            "/groups/{group}/queues/{queue}/messages/receive",
            post(receive_messages),
        )
        .route(
            "/groups/{group}/queues/{queue}/messages/{message}",
            get(get_message).delete(delete_message),
        )
        .route(
            "/groups/{group}/queues/{queue}/messages/{message}/complete",
            post(complete_message),
        )
        .route(
            "/groups/{group}/queues/{queue}/messages/{message}/fail",
            post(fail_message),
        )
        .route(
            "/groups/{group}/queues/{queue}/messages/{message}/requeue",
            post(requeue_message),
        )
}

async fn queue_groups(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_jobs::queue_groups(&s.db).await?))
}

async fn create_group(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Json(body): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_jobs::create_group(&s.db, &body).await?))
}

async fn get_group(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(group): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_jobs::get_group(&s.db, &group).await?.ok_or_else(
        || err(StatusCode::NOT_FOUND, "queue group not found"),
    )?))
}

async fn update_group(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(group): Path<String>,
    Json(body): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(
        sk_jobs::update_group(&s.db, &group, &body)
            .await?
            .ok_or_else(|| err(StatusCode::NOT_FOUND, "queue group not found"))?,
    ))
}

async fn delete_group(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(group): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(
        json!({ "success": sk_jobs::delete_group(&s.db, &group).await? }),
    ))
}

async fn queues(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(group): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_jobs::queues(&s.db, &group).await?))
}

async fn create_queue(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(group): Path<String>,
    Json(body): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_jobs::create_queue(&s.db, &group, &body).await?))
}

async fn get_queue(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path((group, queue)): Path<(String, String)>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(
        sk_jobs::get_queue(&s.db, &group, &queue)
            .await?
            .ok_or_else(|| err(StatusCode::NOT_FOUND, "queue not found"))?,
    ))
}

async fn update_queue(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path((group, queue)): Path<(String, String)>,
    Json(body): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(
        sk_jobs::update_queue(&s.db, &group, &queue, &body)
            .await?
            .ok_or_else(|| err(StatusCode::NOT_FOUND, "queue not found"))?,
    ))
}

async fn delete_queue(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path((group, queue)): Path<(String, String)>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(
        json!({ "success": sk_jobs::delete_queue(&s.db, &group, &queue).await? }),
    ))
}

async fn messages(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path((group, queue)): Path<(String, String)>,
    Query(q): Query<HashMap<String, String>>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(
        sk_jobs::messages(&s.db, &group, &queue, q.get("status").map(String::as_str)).await?,
    ))
}

async fn send_message(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path((group, queue)): Path<(String, String)>,
    Json(body): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(
        sk_jobs::send_message(&s.db, &group, &queue, &body).await?,
    ))
}

async fn receive_messages(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path((group, queue)): Path<(String, String)>,
    Json(body): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(
        sk_jobs::receive_messages(&s.db, &group, &queue, &body).await?,
    ))
}

async fn get_message(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path((group, queue, message)): Path<(String, String, String)>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(
        sk_jobs::get_message(&s.db, &group, &queue, &message)
            .await?
            .ok_or_else(|| err(StatusCode::NOT_FOUND, "message not found"))?,
    ))
}

async fn complete_message(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path((_group, _queue, message)): Path<(String, String, String)>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    sk_jobs::complete_message(&s.db, &message).await?;
    Ok(Json(json!({ "success": true })))
}

async fn fail_message(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path((_group, _queue, message)): Path<(String, String, String)>,
    Json(body): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    sk_jobs::fail_message(&s.db, &message, &body).await?;
    Ok(Json(json!({ "success": true })))
}

async fn requeue_message(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path((_group, _queue, message)): Path<(String, String, String)>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    sk_jobs::requeue_message(&s.db, &message).await?;
    Ok(Json(json!({ "success": true })))
}

async fn delete_message(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path((_group, _queue, message)): Path<(String, String, String)>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(
        json!({ "success": sk_jobs::delete_message(&s.db, &message).await? }),
    ))
}

async fn queue_stats(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path((group, queue)): Path<(String, String)>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(
        sk_jobs::queue_stats(&s.db, Some(&group), Some(&queue)).await?,
    ))
}

async fn group_stats(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(group): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_jobs::queue_stats(&s.db, Some(&group), None).await?))
}

async fn global_queue_stats(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_jobs::queue_stats(&s.db, None, None).await?))
}
