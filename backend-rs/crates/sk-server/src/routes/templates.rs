//! Port of `app/api/templates.py` (catalog + install/uninstall). The
//! deployment-job machinery is replaced by a direct compose deploy.

use crate::error::{ApiError, ApiResult};
use crate::extract::AuthUser;
use crate::state::SharedState;
use axum::extract::{Path, Query};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};

pub fn router() -> Router<SharedState> {
    // Full paths + merged (not nested) so both `/templates` and `/templates/`
    // match — the frontend calls the trailing-slash form.
    Router::new()
        .route("/templates", get(list))
        .route("/templates/", get(list))
        .route("/templates/categories", get(categories))
        .route("/templates/installed", get(installed))
        .route(
            "/templates/installed/{name}",
            axum::routing::delete(uninstall),
        )
        .route("/templates/validate-install", post(validate))
        .route("/templates/{id}", get(detail))
        .route("/templates/{id}/install", post(install))
}

fn require_admin(u: &sk_models::user::User) -> ApiResult<()> {
    if !u.is_admin() {
        return Err(ApiError::forbidden("Admin access required"));
    }
    Ok(())
}

#[derive(Deserialize)]
struct ListQuery {
    category: Option<String>,
    search: Option<String>,
}

async fn list(AuthUser(_u): AuthUser, Query(q): Query<ListQuery>) -> Json<Value> {
    let templates = sk_templates::list(q.category.as_deref(), q.search.as_deref());
    Json(json!({ "count": templates.len(), "templates": templates }))
}

async fn categories(AuthUser(_u): AuthUser) -> Json<Value> {
    Json(json!({ "categories": sk_templates::categories() }))
}

async fn detail(AuthUser(_u): AuthUser, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    match sk_templates::detail(&id) {
        Some(t) => Ok(Json(json!({ "template": t }))),
        None => Err(ApiError::not_found("Template not found")),
    }
}

#[derive(Deserialize)]
struct InstallBody {
    app_name: Option<String>,
    #[serde(default)]
    variables: Value,
}

async fn install(
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
    Json(b): Json<InstallBody>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    require_admin(&u)?;
    let app_name = b
        .app_name
        .ok_or_else(|| ApiError::bad_request("app_name is required"))?;
    let vars = if b.variables.is_object() {
        b.variables
    } else {
        json!({})
    };
    let result = sk_templates::install(&id, &app_name, &vars).await;
    let ok = result["success"].as_bool().unwrap_or(false);
    Ok((
        if ok {
            StatusCode::CREATED
        } else {
            StatusCode::BAD_REQUEST
        },
        Json(result),
    ))
}

#[derive(Deserialize)]
struct ValidateBody {
    template_id: Option<String>,
    app_name: Option<String>,
    #[serde(default)]
    variables: Value,
}

async fn validate(
    AuthUser(_u): AuthUser,
    Json(b): Json<ValidateBody>,
) -> (StatusCode, Json<Value>) {
    let mut errors: Vec<String> = Vec::new();
    match &b.app_name {
        None => errors.push("App name is required".into()),
        Some(n) if !sk_templates::app_name_valid(n) => {
            errors.push("Invalid app name (3-63 chars, lowercase/digits/hyphens)".into())
        }
        _ => {}
    }
    match b.template_id.as_deref().map(sk_templates::detail) {
        None => errors.push("Template ID is required".into()),
        Some(None) => errors.push("Template not found".into()),
        Some(Some(t)) => {
            let provided = b.variables.as_object();
            if let Some(vars) = t["variables"].as_array() {
                for v in vars {
                    if v["required"].as_bool() == Some(true) {
                        let name = v["name"].as_str().unwrap_or("");
                        let has = provided.map(|p| p.contains_key(name)).unwrap_or(false);
                        if !name.is_empty() && !has {
                            errors.push(format!("Required variable \"{name}\" is not provided"));
                        }
                    }
                }
            }
        }
    }
    if errors.is_empty() {
        (StatusCode::OK, Json(json!({ "valid": true })))
    } else {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({ "valid": false, "errors": errors })),
        )
    }
}

async fn installed(AuthUser(_u): AuthUser) -> Json<Value> {
    Json(json!({ "apps": sk_templates::list_installed() }))
}

async fn uninstall(
    AuthUser(u): AuthUser,
    Path(name): Path<String>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    require_admin(&u)?;
    let result = sk_templates::uninstall(&name).await;
    let ok = result["success"].as_bool().unwrap_or(false);
    Ok((
        if ok {
            StatusCode::OK
        } else {
            StatusCode::BAD_REQUEST
        },
        Json(result),
    ))
}
