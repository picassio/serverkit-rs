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

pub fn projects_router() -> Router<SharedState> {
    Router::new()
        .route("/", get(list_projects).post(create_project))
        .route(
            "/{id}",
            get(get_project).put(update_project).delete(delete_project),
        )
}
pub fn environments_router() -> Router<SharedState> {
    Router::new()
        .route("/", post(create_environment))
        .route("/reorder", post(reorder_environments))
        .route(
            "/{id}",
            get(get_environment)
                .put(update_environment)
                .delete(delete_environment),
        )
}
pub fn workspaces_router() -> Router<SharedState> {
    Router::new()
        .route("/", get(list_workspaces).post(create_workspace))
        .route(
            "/{id}",
            get(get_workspace)
                .put(update_workspace)
                .delete(delete_workspace),
        )
        .route("/{id}/archive", post(archive_workspace))
        .route("/{id}/restore", post(restore_workspace))
        .route(
            "/{id}/members",
            get(workspace_members).post(add_workspace_member),
        )
        .route("/members/{id}/role", axum::routing::put(update_member_role))
        .route("/members/{id}", axum::routing::delete(delete_member))
        .route(
            "/{id}/api-keys",
            get(workspace_api_keys).post(create_workspace_api_key),
        )
        .route("/api-keys/{id}/revoke", post(revoke_workspace_api_key))
}
pub fn api_keys_router() -> Router<SharedState> {
    Router::new()
        .route("/", get(list_api_keys).post(create_api_key))
        .route("/scopes", get(api_key_scopes))
        .route(
            "/{id}",
            get(get_api_key).put(update_api_key).delete(revoke_api_key),
        )
        .route("/{id}/rotate", post(rotate_api_key))
}
pub fn vaults_router() -> Router<SharedState> {
    Router::new()
        .route("/", get(list_vaults).post(create_vault))
        .route(
            "/{id}",
            get(get_vault).patch(update_vault).delete(delete_vault),
        )
        .route("/{id}/secrets", get(list_secrets).post(create_secret))
        .route("/{id}/secrets/bulk", post(bulk_create_secrets))
}
pub fn secrets_router() -> Router<SharedState> {
    Router::new()
        .route(
            "/{id}",
            get(get_secret).patch(update_secret).delete(delete_secret),
        )
        .route("/{id}/reveal", post(reveal_secret))
}
pub fn shared_router() -> Router<SharedState> {
    Router::new()
        .route("/resource-types", get(shared_resource_types))
        .route("/tags", get(list_tags).post(add_tag).delete(remove_tag))
        .route(
            "/variable-groups",
            get(list_variable_groups).post(create_variable_group),
        )
        .route(
            "/variable-groups/{id}",
            get(get_variable_group)
                .put(update_variable_group)
                .delete(delete_variable_group),
        )
        .route(
            "/variable-groups/{id}/variables",
            get(group_variables).post(add_variable),
        )
        .route(
            "/variable-groups/{_group_id}/variables/{id}",
            axum::routing::put(update_variable).delete(delete_variable),
        )
        .route("/variable-groups/{id}/attach", post(attach_variable_group))
        .route("/variable-groups/{id}/detach", post(detach_variable_group))
        .route("/resolved", get(resolved_variables))
        .route("/resolved/hierarchical", get(resolved_variables))
}

pub async fn list_projects(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_projects::list_projects(&s.db).await?))
}
pub async fn create_project(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_projects::create_project(&s.db, &b).await?))
}
async fn get_project(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    let mut project = sk_projects::get_project(&s.db, &id)
        .await?
        .ok_or_else(|| nf("project not found"))?;
    let envs = sk_projects::project_environments(&s.db, &id).await?;
    project["environments"] = envs["environments"].clone();
    Ok(Json(json!({"project": project})))
}
async fn update_project(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(
        sk_projects::update_project(&s.db, &id, &b)
            .await?
            .ok_or_else(|| nf("project not found"))?,
    ))
}
async fn delete_project(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(
        json!({"success":sk_projects::delete_project(&s.db,&id).await?}),
    ))
}

async fn create_environment(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_projects::create_environment(&s.db, &b).await?))
}
async fn get_environment(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(
        sk_projects::get_environment(&s.db, &id)
            .await?
            .ok_or_else(|| nf("environment not found"))?,
    ))
}
async fn update_environment(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(
        sk_projects::update_environment(&s.db, &id, &b)
            .await?
            .ok_or_else(|| nf("environment not found"))?,
    ))
}
async fn delete_environment(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(
        json!({"success":sk_projects::delete_environment(&s.db,&id).await?}),
    ))
}
async fn reorder_environments(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_projects::reorder_environments(&s.db, &b).await?))
}

pub async fn list_workspaces(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Query(q): Query<HashMap<String, String>>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(
        sk_projects::list_workspaces(&s.db, q.contains_key("include_archived")).await?,
    ))
}
pub async fn create_workspace(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_projects::create_workspace(&s.db, &b).await?))
}
async fn get_workspace(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(
        sk_projects::get_workspace(&s.db, &id)
            .await?
            .ok_or_else(|| nf("workspace not found"))?,
    ))
}
async fn update_workspace(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(
        sk_projects::update_workspace(&s.db, &id, &b)
            .await?
            .ok_or_else(|| nf("workspace not found"))?,
    ))
}
async fn archive_workspace(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(
        sk_projects::archive_workspace(&s.db, &id, true)
            .await?
            .ok_or_else(|| nf("workspace not found"))?,
    ))
}
async fn restore_workspace(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(
        sk_projects::archive_workspace(&s.db, &id, false)
            .await?
            .ok_or_else(|| nf("workspace not found"))?,
    ))
}
async fn delete_workspace(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(
        json!({"success":sk_projects::delete_workspace(&s.db,&id).await?}),
    ))
}
async fn workspace_members(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_projects::workspace_members(&s.db, &id).await?))
}
async fn add_workspace_member(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(
        sk_projects::add_workspace_member(&s.db, &id, &b).await?,
    ))
}
async fn update_member_role(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(
        json!({"success":sk_projects::update_member_role(&s.db,&id,b.get("role").and_then(Value::as_str).unwrap_or("member")).await?}),
    ))
}
async fn delete_member(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(
        json!({"success":sk_projects::delete_member(&s.db,&id).await?}),
    ))
}
async fn workspace_api_keys(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_projects::list_api_keys(&s.db, Some(&id)).await?))
}
async fn create_workspace_api_key(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(
        sk_projects::create_api_key(&s.db, &b, Some(&id)).await?,
    ))
}
async fn revoke_workspace_api_key(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(
        json!({"success":sk_projects::revoke_api_key(&s.db,&id).await?}),
    ))
}

async fn list_api_keys(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_projects::list_api_keys(&s.db, None).await?))
}
async fn api_key_scopes(AuthUser(u): AuthUser) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_projects::api_key_scopes()))
}
async fn create_api_key(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_projects::create_api_key(&s.db, &b, None).await?))
}
async fn get_api_key(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(
        sk_projects::get_api_key(&s.db, &id)
            .await?
            .ok_or_else(|| nf("api key not found"))?,
    ))
}
async fn update_api_key(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(
        sk_projects::update_api_key(&s.db, &id, &b)
            .await?
            .ok_or_else(|| nf("api key not found"))?,
    ))
}
async fn revoke_api_key(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(
        json!({"success":sk_projects::revoke_api_key(&s.db,&id).await?}),
    ))
}
async fn rotate_api_key(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(
        sk_projects::rotate_api_key(&s.db, &id)
            .await?
            .ok_or_else(|| nf("api key not found"))?,
    ))
}

async fn list_vaults(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_projects::list_vaults(&s.db).await?))
}
async fn create_vault(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_projects::create_vault(&s.db, &b).await?))
}
async fn get_vault(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(
        sk_projects::get_vault(&s.db, &id)
            .await?
            .ok_or_else(|| nf("vault not found"))?,
    ))
}
async fn update_vault(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(
        sk_projects::update_vault(&s.db, &id, &b)
            .await?
            .ok_or_else(|| nf("vault not found"))?,
    ))
}
async fn delete_vault(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(
        json!({"success":sk_projects::delete_vault(&s.db,&id).await?}),
    ))
}
async fn list_secrets(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_projects::list_secrets(&s.db, &id).await?))
}
async fn create_secret(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_projects::create_secret(&s.db, &id, &b).await?))
}
async fn bulk_create_secrets(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    let arr = b
        .get("secrets")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    Ok(Json(
        sk_projects::bulk_create_secrets(&s.db, &id, &arr).await?,
    ))
}
async fn get_secret(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(
        sk_projects::get_secret(&s.db, &id, false)
            .await?
            .ok_or_else(|| nf("secret not found"))?,
    ))
}
async fn reveal_secret(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(
        sk_projects::get_secret(&s.db, &id, true)
            .await?
            .ok_or_else(|| nf("secret not found"))?,
    ))
}
async fn update_secret(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(
        sk_projects::update_secret(&s.db, &id, &b)
            .await?
            .ok_or_else(|| nf("secret not found"))?,
    ))
}
async fn delete_secret(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(
        json!({"success":sk_projects::delete_secret(&s.db,&id).await?}),
    ))
}

async fn shared_resource_types(AuthUser(u): AuthUser) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_projects::shared_resource_types()))
}
async fn list_tags(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Query(q): Query<HashMap<String, String>>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(
        sk_projects::list_tags(
            &s.db,
            q.get("resource_type").map(String::as_str),
            q.get("resource_id").map(String::as_str),
            q.get("tag").map(String::as_str),
        )
        .await?,
    ))
}
async fn add_tag(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_projects::add_tag(&s.db, &b).await?))
}
async fn remove_tag(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_projects::remove_tag(&s.db, &b).await?))
}
async fn list_variable_groups(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Query(q): Query<HashMap<String, String>>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(
        sk_projects::list_variable_groups(
            &s.db,
            q.get("scope_type").map(String::as_str),
            q.get("scope_id").map(String::as_str),
        )
        .await?,
    ))
}
async fn create_variable_group(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_projects::create_variable_group(&s.db, &b).await?))
}
async fn get_variable_group(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(
        sk_projects::get_variable_group(&s.db, &id)
            .await?
            .ok_or_else(|| nf("variable group not found"))?,
    ))
}
async fn update_variable_group(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(
        sk_projects::update_variable_group(&s.db, &id, &b)
            .await?
            .ok_or_else(|| nf("variable group not found"))?,
    ))
}
async fn delete_variable_group(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(
        json!({"success":sk_projects::delete_variable_group(&s.db,&id).await?}),
    ))
}
async fn group_variables(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_projects::group_variables(&s.db, &id).await?))
}
async fn add_variable(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_projects::add_variable(&s.db, &id, &b).await?))
}
async fn update_variable(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path((_group, id)): Path<(String, String)>,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(
        sk_projects::update_variable(&s.db, &id, &b)
            .await?
            .ok_or_else(|| nf("variable not found"))?,
    ))
}
async fn delete_variable(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path((_group, id)): Path<(String, String)>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(
        json!({"success":sk_projects::delete_variable(&s.db,&id).await?}),
    ))
}
async fn attach_variable_group(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(
        sk_projects::attach_variable_group(&s.db, &id, &b).await?,
    ))
}
async fn detach_variable_group(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(
        sk_projects::detach_variable_group(&s.db, &id, &b).await?,
    ))
}
async fn resolved_variables(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Query(q): Query<HashMap<String, String>>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(
        sk_projects::resolved_variables(
            &s.db,
            q.get("resource_type").map(String::as_str).unwrap_or(""),
            q.get("resource_id").map(String::as_str).unwrap_or(""),
        )
        .await?,
    ))
}
