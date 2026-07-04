use crate::error::{ApiError, ApiResult};
use crate::extract::AuthUser;
use crate::state::SharedState;
use axum::extract::{Path, Query, State};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
fn internal(e: anyhow::Error) -> ApiError {
    e.into()
}
fn body(b: Option<Json<Value>>) -> Value {
    b.map(|x| x.0).unwrap_or_else(|| json!({}))
}
#[derive(Deserialize)]
struct RedirectQ {
    redirect_uri: Option<String>,
}
fn status(v: &Value) -> axum::http::StatusCode {
    if v.get("success").and_then(|x| x.as_bool()).unwrap_or(true) {
        axum::http::StatusCode::OK
    } else {
        axum::http::StatusCode::BAD_REQUEST
    }
}
pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/providers", get(providers))
        .route("/authorize/{id}", get(authorize))
        .route("/callback/{id}", post(callback))
        .route("/identities", get(identities))
        .route("/link/{id}", post(link).delete(unlink))
        .route("/admin/config", get(admin_config))
        .route("/admin/config/{id}", put(update_provider))
        .route("/admin/general", put(update_general))
        .route("/admin/test/{id}", post(test_provider))
}
async fn providers(State(s): State<SharedState>, AuthUser(_): AuthUser) -> ApiResult<Json<Value>> {
    Ok(Json(sk_sso::providers(&s.db).await.map_err(internal)?))
}
async fn identities(State(s): State<SharedState>, AuthUser(u): AuthUser) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_sso::identities(&s.db, &u.id.to_string())
            .await
            .map_err(internal)?,
    ))
}
async fn authorize(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
    Query(q): Query<RedirectQ>,
) -> ApiResult<(axum::http::StatusCode, Json<Value>)> {
    let v = sk_sso::authorize(&s.db, &id, q.redirect_uri.as_deref(), None, "login")
        .await
        .map_err(internal)?;
    Ok((status(&v), Json(v)))
}
async fn callback(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
    b: Option<Json<Value>>,
) -> ApiResult<(axum::http::StatusCode, Json<Value>)> {
    let v = sk_sso::callback(&s.db, &id, &body(b))
        .await
        .map_err(internal)?;
    Ok((status(&v), Json(v)))
}
async fn link(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
    b: Option<Json<Value>>,
) -> ApiResult<(axum::http::StatusCode, Json<Value>)> {
    let v = sk_sso::link(&s.db, &id, &body(b)).await.map_err(internal)?;
    let _ = u;
    Ok((status(&v), Json(v)))
}
async fn unlink(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_sso::unlink(&s.db, &u.id.to_string(), &id)
            .await
            .map_err(internal)?,
    ))
}
fn admin(u: &sk_models::user::User) -> ApiResult<()> {
    if !u.is_admin() {
        Err(ApiError::forbidden("Admin access required"))
    } else {
        Ok(())
    }
}
async fn admin_config(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(sk_sso::admin_config(&s.db).await.map_err(internal)?))
}
async fn update_provider(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
    b: Option<Json<Value>>,
) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(
        sk_sso::update_provider(&s.db, &id, &body(b))
            .await
            .map_err(internal)?,
    ))
}
async fn update_general(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    b: Option<Json<Value>>,
) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(
        sk_sso::update_general(&s.db, &body(b))
            .await
            .map_err(internal)?,
    ))
}
async fn test_provider(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(
        sk_sso::test_provider(&s.db, &id).await.map_err(internal)?,
    ))
}
