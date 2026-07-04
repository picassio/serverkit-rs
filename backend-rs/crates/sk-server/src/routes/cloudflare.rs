use crate::error::{ApiError, ApiResult};
use crate::extract::AuthUser;
use crate::state::SharedState;
use axum::extract::{Path, State};
use axum::routing::{delete, get, patch, post};
use axum::{Json, Router};
use serde_json::Value;

fn admin(u: &sk_models::user::User) -> ApiResult<()> {
    if !u.is_admin() {
        Err(ApiError::forbidden("Admin access required"))
    } else {
        Ok(())
    }
}

pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/zones/{zone}/settings", get(settings))
        .route("/zones/{zone}/settings/apply-preset", post(apply_preset))
        .route(
            "/zones/{zone}/settings/{setting}",
            get(setting).patch(update_setting),
        )
        .route("/zones/{zone}/purge-cache", post(purge_cache))
        .route("/zones/{zone}/waf/rules", get(waf_rules).post(add_waf_rule))
        .route("/zones/{zone}/waf/presets/{preset}", post(waf_preset))
        .route(
            "/zones/{zone}/waf/rulesets/{ruleset}/rules/{rule}",
            patch(update_waf_rule).delete(delete_waf_rule),
        )
        .route("/zones/{zone}/workers", get(workers).post(add_worker))
        .route("/zones/{zone}/workers/{name}", delete(delete_worker))
        .route("/zones/{zone}/workers/routes", post(add_worker_route))
        .route(
            "/zones/{zone}/workers/routes/{route}",
            delete(delete_worker_route),
        )
        .route("/zones/{zone}/tunnels", get(tunnels).post(add_tunnel))
        .route("/zones/{zone}/tunnels/{tunnel}", delete(delete_tunnel))
        .route(
            "/zones/{zone}/tunnels/{tunnel}/install",
            get(tunnel_install),
        )
        .route(
            "/zones/{zone}/tunnels/{tunnel}/hostnames",
            get(tunnel_hostnames)
                .post(add_tunnel_hostname)
                .delete(delete_tunnel_hostname),
        )
        .route("/zones/{zone}/storage", get(storage))
        .route("/zones/{zone}/storage/r2", post(add_r2))
        .route("/zones/{zone}/storage/r2/{name}", delete(delete_r2))
        .route("/zones/{zone}/storage/kv", post(add_kv))
        .route("/zones/{zone}/storage/kv/{name}", delete(delete_kv))
        .route("/zones/{zone}/storage/d1", post(add_d1))
        .route("/zones/{zone}/storage/d1/{name}", delete(delete_d1))
}

async fn settings(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(zone): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(sk_cloudflare::settings(&s.db, &zone).await?))
}
async fn setting(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path((zone, setting)): Path<(String, String)>,
) -> ApiResult<Json<Value>> {
    Ok(Json(sk_cloudflare::setting(&s.db, &zone, &setting).await?))
}
async fn update_setting(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path((zone, setting)): Path<(String, String)>,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(
        sk_cloudflare::update_setting(&s.db, &zone, &setting, &b).await?,
    ))
}
async fn apply_preset(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(zone): Path<String>,
) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(sk_cloudflare::apply_preset(&s.db, &zone).await?))
}
async fn purge_cache(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(zone): Path<String>,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(sk_cloudflare::purge_cache(&s.db, &zone, &b).await?))
}
async fn waf_rules(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(zone): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(sk_cloudflare::waf_rules(&s.db, &zone).await?))
}
async fn add_waf_rule(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(zone): Path<String>,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(sk_cloudflare::add_waf_rule(&s.db, &zone, &b).await?))
}
async fn waf_preset(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path((zone, preset)): Path<(String, String)>,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(
        sk_cloudflare::waf_preset(&s.db, &zone, &preset, &b).await?,
    ))
}
async fn update_waf_rule(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path((zone, ruleset, rule)): Path<(String, String, String)>,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(
        sk_cloudflare::update_waf_rule(&s.db, &zone, &ruleset, &rule, &b).await?,
    ))
}
async fn delete_waf_rule(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path((zone, ruleset, rule)): Path<(String, String, String)>,
) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(
        sk_cloudflare::delete_waf_rule(&s.db, &zone, &ruleset, &rule).await?,
    ))
}
async fn workers(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(zone): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(sk_cloudflare::workers(&s.db, &zone).await?))
}
async fn add_worker(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(zone): Path<String>,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(sk_cloudflare::add_worker(&s.db, &zone, &b).await?))
}
async fn delete_worker(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path((zone, name)): Path<(String, String)>,
) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(
        sk_cloudflare::delete_worker(&s.db, &zone, &name).await?,
    ))
}
async fn add_worker_route(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(zone): Path<String>,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(
        sk_cloudflare::add_worker_route(&s.db, &zone, &b).await?,
    ))
}
async fn delete_worker_route(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path((zone, route)): Path<(String, String)>,
) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(
        sk_cloudflare::delete_worker_route(&s.db, &zone, &route).await?,
    ))
}
async fn tunnels(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(zone): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(sk_cloudflare::tunnels(&s.db, &zone).await?))
}
async fn add_tunnel(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(zone): Path<String>,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(sk_cloudflare::add_tunnel(&s.db, &zone, &b).await?))
}
async fn delete_tunnel(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path((zone, tunnel)): Path<(String, String)>,
) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(
        sk_cloudflare::delete_tunnel(&s.db, &zone, &tunnel).await?,
    ))
}
async fn tunnel_install(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path((zone, tunnel)): Path<(String, String)>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_cloudflare::tunnel_install(&s.db, &zone, &tunnel).await?,
    ))
}
async fn tunnel_hostnames(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path((zone, tunnel)): Path<(String, String)>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_cloudflare::tunnel_hostnames(&s.db, &zone, &tunnel).await?,
    ))
}
async fn add_tunnel_hostname(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path((zone, tunnel)): Path<(String, String)>,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(
        sk_cloudflare::add_tunnel_hostname(&s.db, &zone, &tunnel, &b).await?,
    ))
}
async fn delete_tunnel_hostname(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path((zone, tunnel)): Path<(String, String)>,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(
        sk_cloudflare::delete_tunnel_hostname(&s.db, &zone, &tunnel, &b).await?,
    ))
}
async fn storage(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(zone): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(sk_cloudflare::storage(&s.db, &zone).await?))
}
async fn add_r2(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(zone): Path<String>,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(
        sk_cloudflare::add_storage(&s.db, &zone, "r2", &b).await?,
    ))
}
async fn delete_r2(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path((zone, name)): Path<(String, String)>,
) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(
        sk_cloudflare::delete_storage(&s.db, &zone, "r2", &name).await?,
    ))
}
async fn add_kv(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(zone): Path<String>,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(
        sk_cloudflare::add_storage(&s.db, &zone, "kv", &b).await?,
    ))
}
async fn delete_kv(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path((zone, name)): Path<(String, String)>,
) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(
        sk_cloudflare::delete_storage(&s.db, &zone, "kv", &name).await?,
    ))
}
async fn add_d1(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(zone): Path<String>,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(
        sk_cloudflare::add_storage(&s.db, &zone, "d1", &b).await?,
    ))
}
async fn delete_d1(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path((zone, name)): Path<(String, String)>,
) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(
        sk_cloudflare::delete_storage(&s.db, &zone, "d1", &name).await?,
    ))
}
