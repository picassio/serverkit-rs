use crate::error::{ApiError, ApiResult};
use crate::extract::AuthUser;
use crate::state::SharedState;
use axum::extract::{Path, Query, State};
use axum::routing::{delete, get, post, put};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::Value;

fn require_admin(user: &sk_models::user::User) -> ApiResult<()> {
    if !user.is_admin() {
        return Err(ApiError::forbidden("Admin access required"));
    }
    Ok(())
}

pub fn dns_router() -> Router<SharedState> {
    Router::new()
        .route("/", get(zones).post(create_zone))
        .route("/adopt", post(adopt_zone))
        .route("/changes", get(changes))
        .route("/managed", get(managed))
        .route("/portfolio", get(portfolio))
        .route("/presets", get(presets))
        .route("/provider-records", get(provider_records))
        .route("/registration", get(registration))
        .route("/propagation/{domain}", get(propagation))
        .route("/records/{id}", put(update_record).delete(delete_record))
        .route("/{id}", get(get_zone).delete(delete_zone))
        .route("/{id}/apply-preset", post(apply_preset))
        .route("/{id}/export", get(export_zone))
        .route("/{id}/import", post(import_zone))
        .route("/{id}/mirror", get(mirror))
        .route("/{id}/records", get(records).post(create_record))
}

pub fn ddns_router() -> Router<SharedState> {
    Router::new()
        .route("/hosts", get(ddns_hosts).post(create_ddns_host))
        .route("/hosts/{id}", delete(delete_ddns_host))
        .route("/hosts/{id}/regenerate-token", post(regen_ddns_token))
}

pub fn registrars_router() -> Router<SharedState> {
    Router::new()
        .route(
            "/connections",
            get(registrar_connections).post(add_registrar_connection),
        )
        .route("/connections/{id}", delete(delete_registrar_connection))
        .route("/connections/{id}/test", post(test_registrar_connection))
        .route("/domains", get(registrar_domains))
        .route("/sync", post(sync_registrar_domains))
}

pub async fn zones(State(s): State<SharedState>, AuthUser(_u): AuthUser) -> ApiResult<Json<Value>> {
    Ok(Json(sk_dns::zones(&s.db).await?))
}
async fn get_zone(
    State(s): State<SharedState>,
    AuthUser(_u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_dns::get_zone(&s.db, &id).await?.unwrap_or(Value::Null),
    ))
}
pub async fn create_zone(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_dns::create_zone(&s.db, &b).await?))
}
async fn adopt_zone(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_dns::adopt_zone(&s.db, &b).await?))
}
async fn delete_zone(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_dns::delete_zone(&s.db, &id).await?))
}
async fn records(
    State(s): State<SharedState>,
    AuthUser(_u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(sk_dns::records(&s.db, &id).await?))
}
async fn create_record(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_dns::create_record(&s.db, &id, &b).await?))
}
async fn update_record(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_dns::update_record(&s.db, &id, &b).await?))
}
async fn delete_record(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_dns::delete_record(&s.db, &id).await?))
}
async fn presets(AuthUser(_u): AuthUser) -> Json<Value> {
    Json(sk_dns::presets())
}
async fn apply_preset(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_dns::apply_preset(&s.db, &id, &b).await?))
}
async fn export_zone(
    State(s): State<SharedState>,
    AuthUser(_u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(sk_dns::export_zone(&s.db, &id).await?))
}
async fn import_zone(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_dns::import_zone(&s.db, &id, &b).await?))
}
async fn managed(State(s): State<SharedState>, AuthUser(_u): AuthUser) -> ApiResult<Json<Value>> {
    Ok(Json(sk_dns::managed(&s.db).await?))
}
async fn mirror(
    State(s): State<SharedState>,
    AuthUser(_u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(sk_dns::mirror(&s.db, &id).await?))
}
async fn portfolio(State(s): State<SharedState>, AuthUser(_u): AuthUser) -> ApiResult<Json<Value>> {
    Ok(Json(sk_dns::portfolio(&s.db).await?))
}
#[derive(Deserialize)]
struct ProviderRecordsQuery {
    config_id: Option<String>,
    zone: Option<String>,
}
async fn provider_records(
    State(s): State<SharedState>,
    AuthUser(_u): AuthUser,
    Query(q): Query<ProviderRecordsQuery>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_dns::provider_records(&s.db, q.config_id.as_deref(), q.zone.as_deref()).await?,
    ))
}
#[derive(Deserialize)]
struct RegistrationQuery {
    domain: String,
}
async fn registration(
    AuthUser(_u): AuthUser,
    Query(q): Query<RegistrationQuery>,
) -> ApiResult<Json<Value>> {
    Ok(Json(sk_dns::registration(&q.domain).await?))
}
#[derive(Deserialize)]
struct PropagationQuery {
    #[serde(rename = "type")]
    r#type: Option<String>,
}
async fn propagation(
    AuthUser(_u): AuthUser,
    Path(domain): Path<String>,
    Query(q): Query<PropagationQuery>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_dns::propagation(&domain, q.r#type.as_deref().unwrap_or("A")).await?,
    ))
}
#[derive(Deserialize)]
struct ChangesQuery {
    config_id: Option<String>,
    zone: Option<String>,
    result: Option<String>,
    limit: Option<i64>,
}
async fn changes(
    State(s): State<SharedState>,
    AuthUser(_u): AuthUser,
    Query(q): Query<ChangesQuery>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_dns::changes(
            &s.db,
            q.config_id.as_deref(),
            q.zone.as_deref(),
            q.result.as_deref(),
            q.limit.unwrap_or(100),
        )
        .await?,
    ))
}

async fn ddns_hosts(
    State(s): State<SharedState>,
    AuthUser(_u): AuthUser,
) -> ApiResult<Json<Value>> {
    Ok(Json(sk_dns::ddns_hosts(&s.db).await?))
}
async fn create_ddns_host(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_dns::create_ddns_host(&s.db, &b).await?))
}
async fn delete_ddns_host(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_dns::delete_ddns_host(&s.db, &id).await?))
}
async fn regen_ddns_token(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_dns::regen_ddns_token(&s.db, &id).await?))
}

async fn registrar_connections(
    State(s): State<SharedState>,
    AuthUser(_u): AuthUser,
) -> ApiResult<Json<Value>> {
    Ok(Json(sk_dns::registrar_connections(&s.db).await?))
}
async fn add_registrar_connection(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_dns::add_registrar_connection(&s.db, &b).await?))
}
async fn delete_registrar_connection(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_dns::delete_registrar_connection(&s.db, &id).await?))
}
async fn test_registrar_connection(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_dns::test_registrar_connection(&s.db, &id).await?))
}
async fn registrar_domains(
    State(s): State<SharedState>,
    AuthUser(_u): AuthUser,
) -> ApiResult<Json<Value>> {
    Ok(Json(sk_dns::registrar_domains(&s.db).await?))
}
async fn sync_registrar_domains(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_dns::sync_registrar_domains(&s.db).await?))
}
