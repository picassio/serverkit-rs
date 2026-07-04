use crate::error::{ApiError, ApiResult};
use crate::extract::AuthUser;
use crate::state::SharedState;
use axum::extract::{Path, Query, State};
use axum::routing::{delete, get, post, put};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
fn internal(e: anyhow::Error) -> ApiError {
    e.into()
}
fn body(b: Option<Json<Value>>) -> Value {
    match b.map(|x| x.0).unwrap_or_else(|| json!({})) {
        Value::String(s) => serde_json::from_str(&s).unwrap_or_else(|_| json!({"value":s})),
        v => v,
    }
}
#[derive(Deserialize)]
struct Lines {
    lines: Option<i64>,
}
pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/status", get(status))
        .route("/install", post(install))
        .route("/service/{component}/{action}", post(control))
        .route("/domains", get(domains).post(add_domain))
        .route("/domains/{id}", get(domain).delete(delete_domain))
        .route("/domains/{id}/verify-dns", post(verify_dns))
        .route("/domains/{id}/deploy-dns", post(deploy_dns))
        .route("/domains/{id}/accounts", get(accounts).post(create_account))
        .route(
            "/accounts/{id}",
            get(account).put(update_account).delete(delete_account),
        )
        .route("/accounts/{id}/password", post(change_password))
        .route("/domains/{id}/aliases", get(aliases).post(create_alias))
        .route("/aliases/{id}", delete(delete_alias))
        .route(
            "/accounts/{id}/forwarding",
            get(forwarding).post(create_forwarding),
        )
        .route(
            "/forwarding/{id}",
            put(update_forwarding).delete(delete_forwarding),
        )
        .route("/dns-providers", get(providers).post(add_provider))
        .route("/dns-providers/{id}", delete(delete_provider))
        .route("/dns-providers/{id}/test", post(test_provider))
        .route("/dns-providers/{id}/zones", get(zones))
        .route("/relay", get(relay).put(update_relay).delete(disable_relay))
        .route("/relay/test", post(test_relay))
        .route("/spam/config", get(spam).put(update_spam))
        .route("/spam/update-rules", post(update_spam_rules))
        .route("/webmail/status", get(webmail_status))
        .route("/webmail/install", post(webmail_install))
        .route("/webmail/service/{action}", post(webmail_control))
        .route("/webmail/configure-proxy", post(webmail_proxy))
        .route("/queue", get(queue))
        .route("/queue/flush", post(flush_queue))
        .route("/queue/{id}", delete(delete_queue))
        .route("/logs", get(logs))
}
async fn status(State(s): State<SharedState>, AuthUser(_): AuthUser) -> ApiResult<Json<Value>> {
    Ok(Json(sk_email::status(&s.db).await.map_err(internal)?))
}
async fn install(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    b: Option<Json<Value>>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_email::install(&s.db, &body(b)).await.map_err(internal)?,
    ))
}
async fn control(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path((component, action)): Path<(String, String)>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_email::control(&s.db, &component, &action)
            .await
            .map_err(internal)?,
    ))
}
async fn domains(State(s): State<SharedState>, AuthUser(_): AuthUser) -> ApiResult<Json<Value>> {
    Ok(Json(sk_email::domains(&s.db).await.map_err(internal)?))
}
async fn add_domain(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    b: Option<Json<Value>>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_email::add_domain(&s.db, &body(b))
            .await
            .map_err(internal)?,
    ))
}
async fn domain(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(sk_email::domain(&s.db, &id).await.map_err(internal)?))
}
async fn delete_domain(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_email::delete_domain(&s.db, &id)
            .await
            .map_err(internal)?,
    ))
}
async fn verify_dns(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_email::verify_dns(&s.db, &id).await.map_err(internal)?,
    ))
}
async fn deploy_dns(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_email::deploy_dns(&s.db, &id).await.map_err(internal)?,
    ))
}
async fn accounts(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_email::accounts(&s.db, &id).await.map_err(internal)?,
    ))
}
async fn create_account(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
    b: Option<Json<Value>>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_email::create_account(&s.db, &id, &body(b))
            .await
            .map_err(internal)?,
    ))
}
async fn account(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(sk_email::account(&s.db, &id).await.map_err(internal)?))
}
async fn update_account(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
    b: Option<Json<Value>>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_email::update_account(&s.db, &id, &body(b))
            .await
            .map_err(internal)?,
    ))
}
async fn delete_account(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_email::delete_account(&s.db, &id)
            .await
            .map_err(internal)?,
    ))
}
async fn change_password(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
    b: Option<Json<Value>>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_email::change_password(&s.db, &id, &body(b))
            .await
            .map_err(internal)?,
    ))
}
async fn aliases(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(sk_email::aliases(&s.db, &id).await.map_err(internal)?))
}
async fn create_alias(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
    b: Option<Json<Value>>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_email::create_alias(&s.db, &id, &body(b))
            .await
            .map_err(internal)?,
    ))
}
async fn delete_alias(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_email::delete_alias(&s.db, &id).await.map_err(internal)?,
    ))
}
async fn forwarding(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_email::forwarding(&s.db, &id).await.map_err(internal)?,
    ))
}
async fn create_forwarding(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
    b: Option<Json<Value>>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_email::create_forwarding(&s.db, &id, &body(b))
            .await
            .map_err(internal)?,
    ))
}
async fn update_forwarding(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
    b: Option<Json<Value>>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_email::update_forwarding(&s.db, &id, &body(b))
            .await
            .map_err(internal)?,
    ))
}
async fn delete_forwarding(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_email::delete_forwarding(&s.db, &id)
            .await
            .map_err(internal)?,
    ))
}
async fn providers(State(s): State<SharedState>, AuthUser(_): AuthUser) -> ApiResult<Json<Value>> {
    Ok(Json(sk_email::providers(&s.db).await.map_err(internal)?))
}
async fn add_provider(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    b: Option<Json<Value>>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_email::add_provider(&s.db, &body(b))
            .await
            .map_err(internal)?,
    ))
}
async fn delete_provider(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_email::delete_provider(&s.db, &id)
            .await
            .map_err(internal)?,
    ))
}
async fn test_provider(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_email::test_provider(&s.db, &id)
            .await
            .map_err(internal)?,
    ))
}
async fn zones(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(sk_email::zones(&s.db, &id).await.map_err(internal)?))
}
async fn relay(State(s): State<SharedState>, AuthUser(_): AuthUser) -> ApiResult<Json<Value>> {
    Ok(Json(sk_email::relay(&s.db).await.map_err(internal)?))
}
async fn update_relay(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    b: Option<Json<Value>>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_email::update_relay(&s.db, &body(b))
            .await
            .map_err(internal)?,
    ))
}
async fn disable_relay(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_email::disable_relay(&s.db).await.map_err(internal)?,
    ))
}
async fn test_relay(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    b: Option<Json<Value>>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_email::test_relay(&s.db, &body(b))
            .await
            .map_err(internal)?,
    ))
}
async fn spam(State(s): State<SharedState>, AuthUser(_): AuthUser) -> ApiResult<Json<Value>> {
    Ok(Json(sk_email::spam(&s.db).await.map_err(internal)?))
}
async fn update_spam(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    b: Option<Json<Value>>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_email::update_spam(&s.db, &body(b))
            .await
            .map_err(internal)?,
    ))
}
async fn update_spam_rules(AuthUser(_): AuthUser) -> ApiResult<Json<Value>> {
    Ok(Json(sk_email::update_spam_rules().await.map_err(internal)?))
}
async fn webmail_status(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_email::webmail_status(&s.db).await.map_err(internal)?,
    ))
}
async fn webmail_install(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_email::webmail_install(&s.db).await.map_err(internal)?,
    ))
}
async fn webmail_control(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(action): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_email::webmail_control(&s.db, &action)
            .await
            .map_err(internal)?,
    ))
}
async fn webmail_proxy(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    b: Option<Json<Value>>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_email::webmail_proxy(&s.db, &body(b))
            .await
            .map_err(internal)?,
    ))
}
async fn queue(State(s): State<SharedState>, AuthUser(_): AuthUser) -> ApiResult<Json<Value>> {
    Ok(Json(sk_email::queue(&s.db).await.map_err(internal)?))
}
async fn flush_queue(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
) -> ApiResult<Json<Value>> {
    Ok(Json(sk_email::flush_queue(&s.db).await.map_err(internal)?))
}
async fn delete_queue(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_email::delete_queue(&s.db, &id).await.map_err(internal)?,
    ))
}
async fn logs(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Query(q): Query<Lines>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_email::logs(&s.db, q.lines.unwrap_or(100))
            .await
            .map_err(internal)?,
    ))
}
