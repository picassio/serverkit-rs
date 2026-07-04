use crate::error::{ApiError, ApiResult};
use crate::extract::AuthUser;
use crate::state::SharedState;
use axum::extract::{Path, State};
use axum::routing::{delete, get, post, put};
use axum::{Json, Router};
use serde_json::Value;

fn require_admin(user: &sk_models::user::User) -> ApiResult<()> {
    if !user.is_admin() {
        return Err(ApiError::forbidden("Admin access required"));
    }
    Ok(())
}

pub fn node_router() -> Router<SharedState> {
    Router::new()
        .route("/versions", get(node_versions))
        .route("/apps", post(create_node_app))
        .route("/apps/{id}", delete(delete_node_app))
        .route(
            "/apps/{id}/packages",
            get(get_node_packages).post(install_node_packages),
        )
        .route("/apps/{id}/env", get(get_node_env).put(set_node_env))
        .route("/apps/{id}/env/{key}", delete(delete_node_env))
        .route("/apps/{id}/start", post(start_node_app))
        .route("/apps/{id}/stop", post(stop_node_app))
        .route("/apps/{id}/restart", post(restart_node_app))
        .route("/apps/{id}/status", get(node_app_status))
        .route("/apps/{id}/start-command", put(set_node_start_command))
        .route("/apps/{id}/run", post(run_node_command))
}

pub fn python_router() -> Router<SharedState> {
    Router::new()
        .route("/versions", get(python_versions))
        .route("/apps/flask", post(create_flask_app))
        .route("/apps/django", post(create_django_app))
        .route("/apps/{id}", delete(delete_python_app))
        .route("/apps/{id}/venv", post(create_venv))
        .route(
            "/apps/{id}/packages",
            get(get_packages).post(install_packages),
        )
        .route("/apps/{id}/requirements", post(freeze_requirements))
        .route("/apps/{id}/env", get(get_env).put(set_env))
        .route("/apps/{id}/env/{key}", delete(delete_env))
        .route("/apps/{id}/start", post(start_app))
        .route("/apps/{id}/stop", post(stop_app))
        .route("/apps/{id}/restart", post(restart_app))
        .route("/apps/{id}/status", get(app_status))
        .route(
            "/apps/{id}/gunicorn",
            get(gunicorn_config).put(set_gunicorn_config),
        )
        .route("/apps/{id}/migrate", post(migrate))
        .route("/apps/{id}/collectstatic", post(collectstatic))
        .route("/apps/{id}/run", post(run_command))
}

async fn node_versions(AuthUser(u): AuthUser) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_runtimes::node_versions().await))
}
async fn create_node_app(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Json(body): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_runtimes::create_node_app(&s.db, &body).await?))
}
async fn delete_node_app(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(
        sk_runtimes::delete_node_app(
            &s.db,
            &id,
            body.get("remove_files")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        )
        .await?,
    ))
}
async fn get_node_packages(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_runtimes::node_packages(&s.db, &id).await?))
}
async fn install_node_packages(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(
        sk_runtimes::install_node_packages(&s.db, &id, &body).await?,
    ))
}
async fn get_node_env(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_runtimes::node_env_vars(&s.db, &id).await?))
}
async fn set_node_env(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(
        sk_runtimes::set_node_env_vars(&s.db, &id, &body).await?,
    ))
}
async fn delete_node_env(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path((id, key)): Path<(String, String)>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(
        sk_runtimes::delete_node_env_var(&s.db, &id, &key).await?,
    ))
}
async fn set_node_start_command(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(
        sk_runtimes::set_node_start_command(&s.db, &id, &body).await?,
    ))
}
async fn start_node_app(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_runtimes::start_node_app(&s.db, &id).await?))
}
async fn stop_node_app(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_runtimes::stop_node_app(&s.db, &id).await?))
}
async fn restart_node_app(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_runtimes::restart_node_app(&s.db, &id).await?))
}
async fn node_app_status(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_runtimes::node_status(&s.db, &id).await?))
}
async fn run_node_command(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(
        sk_runtimes::run_node_command(&s.db, &id, &body).await?,
    ))
}

async fn python_versions(AuthUser(u): AuthUser) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_runtimes::python_versions().await))
}
async fn create_flask_app(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Json(body): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_runtimes::create_app(&s.db, "flask", &body).await?))
}
async fn create_django_app(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Json(body): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_runtimes::create_app(&s.db, "django", &body).await?))
}
async fn delete_python_app(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(
        sk_runtimes::delete_app(
            &s.db,
            &id,
            body.get("remove_files")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        )
        .await?,
    ))
}
async fn create_venv(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_runtimes::create_venv(&s.db, &id).await?))
}
async fn get_packages(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_runtimes::packages(&s.db, &id).await?))
}
async fn install_packages(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(
        sk_runtimes::install_packages(&s.db, &id, &body).await?,
    ))
}
async fn freeze_requirements(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_runtimes::freeze_requirements(&s.db, &id).await?))
}
async fn get_env(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_runtimes::env_vars(&s.db, &id).await?))
}
async fn set_env(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_runtimes::set_env_vars(&s.db, &id, &body).await?))
}
async fn delete_env(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path((id, key)): Path<(String, String)>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_runtimes::delete_env_var(&s.db, &id, &key).await?))
}
async fn start_app(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_runtimes::start_app(&s.db, &id).await?))
}
async fn stop_app(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_runtimes::stop_app(&s.db, &id).await?))
}
async fn restart_app(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_runtimes::restart_app(&s.db, &id).await?))
}
async fn app_status(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_runtimes::status(&s.db, &id).await?))
}
async fn gunicorn_config(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_runtimes::gunicorn_config(&s.db, &id).await?))
}
async fn set_gunicorn_config(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(
        sk_runtimes::set_gunicorn_config(&s.db, &id, &body).await?,
    ))
}
async fn migrate(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(
        sk_runtimes::django_action(&s.db, &id, "migrate").await?,
    ))
}
async fn collectstatic(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(
        sk_runtimes::django_action(&s.db, &id, "collectstatic").await?,
    ))
}
async fn run_command(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_runtimes::run_app_command(&s.db, &id, &body).await?))
}
