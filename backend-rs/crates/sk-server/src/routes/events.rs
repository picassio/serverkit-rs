use crate::error::{ApiError, ApiResult};
use crate::extract::AuthUser;
use crate::state::SharedState;
use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::{Request, StatusCode};
use axum::middleware::Next;
use axum::response::Response;
use axum::routing::{delete, get, post, put};
use axum::{Json, Router};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::time::Instant;

fn require_admin(u: &sk_models::user::User) -> ApiResult<()> {
    if !u.is_admin() {
        return Err(ApiError::forbidden("Admin access required"));
    }
    Ok(())
}
fn nf(msg: &str) -> ApiError {
    ApiError::new(StatusCode::NOT_FOUND, msg)
}

pub async fn api_analytics_middleware(
    State(state): State<SharedState>,
    req: Request<Body>,
    next: Next,
) -> Response {
    let started = Instant::now();
    let method = req.method().as_str().to_string();
    let path = req
        .uri()
        .path()
        .strip_prefix("/api/v1")
        .unwrap_or(req.uri().path())
        .to_string();
    let response = next.run(req).await;
    let status = response.status().as_u16();
    let latency_ms = started.elapsed().as_millis().min(i64::MAX as u128) as i64;
    let error = (status >= 400).then(|| response.status().to_string());
    let db = state.db.clone();
    tokio::spawn(async move {
        if let Err(e) =
            sk_events::record_api_request(&db, &method, &path, status, latency_ms, error.as_deref())
                .await
        {
            tracing::debug!(error = %e, "failed to record api analytics");
        }
    });
    response
}

pub fn telemetry_router() -> Router<SharedState> {
    Router::new()
        .route("/events", get(events).delete(cleanup_events))
        .route("/events/test", post(test_event))
        .route("/events/{id}", get(event))
        .route("/events/by-correlation/{id}", get(events_by_correlation))
        .route("/stats", get(telemetry_stats))
        .route("/sources", get(sources))
        .route("/event-types", get(event_types))
}

pub fn notifications_router() -> Router<SharedState> {
    Router::new()
        .route("/status", get(notification_status))
        .route("/config", get(notification_config))
        .route("/config/{id}", put(put_notification_config))
        .route("/preferences", get(preferences).put(put_preferences))
        .route("/preferences/test", post(test_notification))
        .route("/test", post(test_notification))
        .route("/test/{id}", post(test_notification_id))
        .route("/inbox", get(inbox))
        .route("/inbox/unread-count", get(unread_count))
        .route("/inbox/read-all", post(mark_all_read))
        .route("/inbox/{id}/read", post(mark_read))
        .route("/admin/deliveries", get(delivery_log))
        .route("/admin/deliveries/{id}/retry", post(retry_delivery))
        .route(
            "/admin/email-providers",
            get(email_providers).post(add_email_provider),
        )
        .route("/admin/email-providers/{id}", delete(delete_email_provider))
        .route(
            "/admin/email-providers/{id}/test",
            post(test_email_provider),
        )
        .route(
            "/admin/email-providers/{id}/default",
            post(default_email_provider),
        )
}

pub fn event_subscriptions_router() -> Router<SharedState> {
    Router::new()
        .route("/", get(subscriptions).post(create_subscription))
        .route("/events", get(available_events))
        .route(
            "/{id}",
            get(subscription)
                .put(update_subscription)
                .delete(delete_subscription),
        )
        .route("/{id}/test", post(test_subscription))
        .route("/{id}/deliveries", get(subscription_deliveries))
        .route("/{_sub}/deliveries/{id}/retry", post(retry_delivery))
}

pub fn webhooks_router() -> Router<SharedState> {
    Router::new()
        .route(
            "/endpoints",
            get(webhook_endpoints).post(create_webhook_endpoint),
        )
        .route(
            "/endpoints/{id}",
            get(webhook_endpoint)
                .patch(update_webhook_endpoint)
                .delete(delete_webhook_endpoint),
        )
        .route(
            "/endpoints/{id}/regenerate-secret",
            post(regenerate_webhook_secret),
        )
        .route("/endpoints/{id}/deliveries", get(webhook_deliveries))
        .route("/deliveries/{id}/replay", post(retry_delivery))
}

pub fn api_analytics_router() -> Router<SharedState> {
    Router::new()
        .route("/overview", get(api_analytics_overview))
        .route("/endpoints", get(api_analytics_endpoints))
        .route("/errors", get(api_analytics_errors))
        .route("/timeseries", get(api_analytics_timeseries))
        .route("/keys/{id}/usage", get(api_key_usage))
}

async fn events(State(s): State<SharedState>, AuthUser(u): AuthUser) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_events::list_events(&s.db).await?))
}
async fn test_event(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_events::emit_event(&s.db, &b).await?))
}
async fn event(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(
        sk_events::get_event(&s.db, &id)
            .await?
            .ok_or_else(|| nf("event not found"))?,
    ))
}
async fn events_by_correlation(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_events::events_by_correlation(&s.db, &id).await?))
}
async fn telemetry_stats(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_events::telemetry_stats(&s.db).await?))
}
async fn sources(State(s): State<SharedState>, AuthUser(u): AuthUser) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_events::event_sources(&s.db).await?))
}
async fn event_types(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_events::event_types(&s.db).await?))
}
async fn cleanup_events(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Query(q): Query<HashMap<String, String>>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    let days = q.get("days").and_then(|x| x.parse().ok()).unwrap_or(90);
    Ok(Json(sk_events::cleanup_events(&s.db, days).await?))
}

async fn inbox(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Query(q): Query<HashMap<String, String>>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(
        sk_events::inbox(&s.db, q.contains_key("unread")).await?,
    ))
}
async fn unread_count(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_events::unread_count(&s.db).await?))
}
async fn mark_read(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_events::mark_read(&s.db, &id).await?))
}
async fn mark_all_read(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_events::mark_all_read(&s.db).await?))
}
async fn notification_status(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_events::notification_status(&s.db).await?))
}
async fn notification_config(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_events::notification_config(&s.db).await?))
}
async fn put_notification_config(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(
        sk_events::put_notification_config(&s.db, &id, &b).await?,
    ))
}
async fn preferences(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_events::preferences(&s.db).await?))
}
async fn put_preferences(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(
        sk_events::put_notification_config(&s.db, "preferences", &b).await?,
    ))
}
async fn test_notification(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    body: Option<Json<Value>>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    let title = body
        .as_ref()
        .and_then(|b| b.0.get("title").and_then(Value::as_str))
        .unwrap_or("Test notification");
    Ok(Json(
        sk_events::create_notification(&s.db, title, "Test notification", "info").await?,
    ))
}
async fn test_notification_id(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(
        sk_events::create_notification(&s.db, &format!("Test {id}"), "Test notification", "info")
            .await?,
    ))
}
async fn delivery_log(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_events::delivery_log(&s.db).await?))
}
async fn retry_delivery(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_events::retry_delivery(&s.db, &id).await?))
}
async fn email_providers(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_events::email_providers(&s.db).await?))
}
async fn add_email_provider(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_events::add_email_provider(&s.db, &b).await?))
}
async fn delete_email_provider(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_events::delete_email_provider(&s.db, &id).await?))
}
async fn default_email_provider(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_events::set_default_provider(&s.db, &id).await?))
}
async fn test_email_provider(
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(
        json!({"success":true,"provider_id":id,"delivered":false,"message":"Provider configuration accepted; outbound SMTP worker not yet enabled"}),
    ))
}

async fn available_events(AuthUser(u): AuthUser) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_events::available_events()))
}
async fn subscriptions(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_events::subscriptions(&s.db).await?))
}
async fn create_subscription(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_events::create_subscription(&s.db, &b).await?))
}
async fn subscription(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(
        sk_events::get_subscription(&s.db, &id)
            .await?
            .ok_or_else(|| nf("subscription not found"))?,
    ))
}
async fn update_subscription(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(
        sk_events::update_subscription(&s.db, &id, &b)
            .await?
            .ok_or_else(|| nf("subscription not found"))?,
    ))
}
async fn delete_subscription(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_events::delete_subscription(&s.db, &id).await?))
}
async fn test_subscription(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_events::test_subscription(&s.db, &id).await?))
}
async fn subscription_deliveries(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_events::subscription_deliveries(&s.db, &id).await?))
}

async fn webhook_endpoints(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_events::webhook_endpoints(&s.db).await?))
}
async fn create_webhook_endpoint(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_events::create_webhook_endpoint(&s.db, &b).await?))
}
async fn webhook_endpoint(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(
        sk_events::get_webhook_endpoint(&s.db, &id)
            .await?
            .ok_or_else(|| nf("webhook endpoint not found"))?,
    ))
}
async fn update_webhook_endpoint(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(
        sk_events::update_webhook_endpoint(&s.db, &id, &b)
            .await?
            .ok_or_else(|| nf("webhook endpoint not found"))?,
    ))
}
async fn delete_webhook_endpoint(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_events::delete_webhook_endpoint(&s.db, &id).await?))
}
async fn regenerate_webhook_secret(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(
        sk_events::regenerate_webhook_secret(&s.db, &id).await?,
    ))
}
async fn webhook_deliveries(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_events::webhook_deliveries(&s.db, &id).await?))
}

async fn api_analytics_overview(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_events::api_analytics_overview(&s.db).await?))
}
async fn api_analytics_endpoints(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_events::api_analytics_endpoints(&s.db).await?))
}
async fn api_analytics_errors(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_events::api_analytics_errors(&s.db).await?))
}
async fn api_analytics_timeseries(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_events::api_analytics_timeseries(&s.db).await?))
}
async fn api_key_usage(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_events::api_key_usage(&s.db, &id).await?))
}
