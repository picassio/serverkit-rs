use crate::error::{ApiError, ApiResult};
use crate::extract::AuthUser;
use crate::state::SharedState;
use axum::extract::{Path, Query, State};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};

fn admin(u: &sk_models::user::User) -> ApiResult<()> {
    if !u.is_admin() {
        Err(ApiError::forbidden("Admin access required"))
    } else {
        Ok(())
    }
}

pub fn firewall_router() -> Router<SharedState> {
    Router::new()
        .route("/status", get(firewall_status))
        .route(
            "/rules",
            get(firewall_rules)
                .post(firewall_add_rule)
                .delete(firewall_del_rule),
        )
        .route("/enable", post(firewall_enable))
        .route("/disable", post(firewall_disable))
        .route("/install", post(firewall_install))
        .route("/block-ip", post(block_ip))
        .route("/unblock-ip", post(unblock_ip))
        .route("/blocked-ips", get(blocked_ips))
        .route("/allow-port", post(allow_port))
        .route("/deny-port", post(deny_port))
        .route("/zones", get(firewall_zones))
        .route("/zones/default", post(set_default_zone))
}

pub fn security_router() -> Router<SharedState> {
    Router::new()
        .route("/status", get(status))
        .route("/config", get(config).put(set_config))
        .route("/audit", get(audit))
        .route("/events", get(events))
        .route("/clamav/status", get(clamav_status))
        .route("/clamav/install", post(clamav_install))
        .route("/clamav/update", post(clamav_update))
        .route("/clamav/start", post(clamav_start))
        .route("/scan/file", post(scan_file))
        .route("/scan/directory", post(scan_directory))
        .route("/scan/status", get(scan_status))
        .route("/scan/cancel", post(scan_cancel))
        .route("/scan/history", get(scan_history))
        .route("/scan/quick", post(scan_quick))
        .route("/scan/full", post(scan_full))
        .route("/quarantine", get(quarantine_list).post(quarantine_add))
        .route("/quarantine/{id}", delete(quarantine_delete))
        .route("/integrity/initialize", post(integrity_initialize))
        .route("/integrity/check", get(integrity_check))
        .route("/failed-logins", get(failed_logins))
        .route("/fail2ban/status", get(fail2ban_status))
        .route("/fail2ban/install", post(fail2ban_install))
        .route("/fail2ban/jails/{jail}", get(fail2ban_jail))
        .route("/fail2ban/bans", get(fail2ban_bans))
        .route("/fail2ban/ban", post(fail2ban_ban))
        .route("/fail2ban/unban", post(fail2ban_unban))
        .route("/ssh-keys", get(ssh_keys).post(ssh_add_key))
        .route("/ssh-keys/{id}", delete(ssh_delete_key))
        .route("/ip-lists", get(ip_lists))
        .route("/ip-lists/{list}", post(ip_list_add))
        .route("/ip-lists/{list}/{ip}", delete(ip_list_delete))
        .route("/lynis/status", get(lynis_status))
        .route("/lynis/install", post(lynis_install))
        .route("/lynis/scan", post(lynis_scan))
        .route("/lynis/scan/status", get(lynis_scan_status))
        .route("/auto-updates/status", get(auto_updates_status))
        .route("/auto-updates/install", post(auto_updates_install))
        .route("/auto-updates/enable", post(auto_updates_enable))
        .route("/auto-updates/disable", post(auto_updates_disable))
}

pub fn waf_router() -> Router<SharedState> {
    Router::new()
        .route("/status", get(waf_status))
        .route("/install", post(waf_install))
        .route(
            "/applications/{id}/policy",
            get(waf_policy).put(waf_set_policy),
        )
        .route("/applications/{id}/apply", post(waf_apply))
        .route("/applications/{id}/events", get(waf_events))
}

async fn firewall_status(AuthUser(_): AuthUser) -> Json<Value> {
    Json(sk_security::firewall_status().await)
}
async fn firewall_rules(AuthUser(_): AuthUser) -> Json<Value> {
    Json(sk_security::firewall_rules().await)
}
async fn firewall_enable(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(sk_security::firewall_enable(&s.db).await?))
}
async fn firewall_disable(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(sk_security::firewall_disable(&s.db).await?))
}
async fn firewall_install(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(sk_security::firewall_install(&s.db).await?))
}
async fn firewall_add_rule(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(sk_security::firewall_add_rule(&s.db, &b).await?))
}
async fn firewall_del_rule(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(sk_security::firewall_del_rule(&s.db, &b).await?))
}
async fn block_ip(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(
        sk_security::firewall_block_ip(&s.db, b["ip"].as_str().unwrap_or("")).await?,
    ))
}
async fn unblock_ip(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(
        sk_security::firewall_unblock_ip(&s.db, b["ip"].as_str().unwrap_or("")).await?,
    ))
}
async fn blocked_ips(AuthUser(_): AuthUser) -> Json<Value> {
    Json(sk_security::blocked_ips().await)
}
async fn allow_port(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(sk_security::firewall_add_rule(&s.db,&json!({"action":"allow","port":b["port"],"protocol":b.get("protocol").cloned().unwrap_or(json!("tcp"))})).await?))
}
async fn deny_port(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(sk_security::firewall_add_rule(&s.db,&json!({"action":"deny","port":b["port"],"protocol":b.get("protocol").cloned().unwrap_or(json!("tcp"))})).await?))
}
async fn firewall_zones(AuthUser(_): AuthUser) -> Json<Value> {
    Json(sk_security::zones())
}
async fn set_default_zone(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(
        sk_security::set_default_zone(&s.db, b["zone"].as_str().unwrap_or("public")).await?,
    ))
}

async fn status(State(s): State<SharedState>, AuthUser(_): AuthUser) -> ApiResult<Json<Value>> {
    Ok(Json(sk_security::status(&s.db).await?))
}
async fn config(State(s): State<SharedState>, AuthUser(_): AuthUser) -> ApiResult<Json<Value>> {
    Ok(Json(sk_security::config(&s.db).await?))
}
async fn set_config(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(sk_security::set_config(&s.db, &b).await?))
}
async fn audit(State(s): State<SharedState>, AuthUser(_): AuthUser) -> ApiResult<Json<Value>> {
    Ok(Json(sk_security::audit(&s.db).await?))
}
#[derive(Deserialize)]
struct Limit {
    limit: Option<i64>,
}
async fn events(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Query(q): Query<Limit>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_security::events(&s.db, q.limit.unwrap_or(100)).await?,
    ))
}
async fn clamav_status(AuthUser(_): AuthUser) -> Json<Value> {
    Json(sk_security::clamav_status().await)
}
async fn clamav_install(AuthUser(u): AuthUser) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(sk_security::clamav_install().await))
}
async fn clamav_update(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(sk_security::clamav_update(&s.db).await?))
}
async fn clamav_start(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(sk_security::clamav_start(&s.db).await?))
}
async fn scan_file(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(
        sk_security::scan_path(&s.db, "file", b["path"].as_str().unwrap_or(""), false).await?,
    ))
}
async fn scan_directory(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(
        sk_security::scan_path(
            &s.db,
            "directory",
            b["path"].as_str().unwrap_or(""),
            b["recursive"].as_bool().unwrap_or(true),
        )
        .await?,
    ))
}
async fn scan_status(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
) -> ApiResult<Json<Value>> {
    Ok(Json(sk_security::scan_status(&s.db).await?))
}
async fn scan_cancel(AuthUser(u): AuthUser) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(sk_security::scan_cancel().await))
}
#[derive(Deserialize)]
struct Hist {
    limit: Option<i64>,
}
async fn scan_history(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Query(q): Query<Hist>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_security::scan_history(&s.db, q.limit.unwrap_or(50)).await?,
    ))
}
async fn scan_quick(State(s): State<SharedState>, AuthUser(u): AuthUser) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(sk_security::scan_quick(&s.db).await?))
}
async fn scan_full(State(s): State<SharedState>, AuthUser(u): AuthUser) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(sk_security::scan_full(&s.db).await?))
}
async fn quarantine_list(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
) -> ApiResult<Json<Value>> {
    Ok(Json(sk_security::quarantine_list(&s.db).await?))
}
async fn quarantine_add(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(
        sk_security::quarantine_add(&s.db, b["path"].as_str().unwrap_or("")).await?,
    ))
}
async fn quarantine_delete(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(sk_security::quarantine_delete(&s.db, &id).await?))
}
async fn integrity_initialize(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    admin(&u)?;
    let paths: Vec<String> = b
        .get("paths")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_else(|| vec!["/var/www".into()]);
    Ok(Json(
        sk_security::integrity_initialize(&s.db, &paths).await?,
    ))
}
async fn integrity_check(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
) -> ApiResult<Json<Value>> {
    Ok(Json(sk_security::integrity_check(&s.db).await?))
}
#[derive(Deserialize)]
struct Hours {
    hours: Option<i64>,
}
async fn failed_logins(AuthUser(_): AuthUser, Query(q): Query<Hours>) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_security::failed_logins(q.hours.unwrap_or(24)).await?,
    ))
}
async fn fail2ban_status(AuthUser(_): AuthUser) -> Json<Value> {
    Json(sk_security::fail2ban_status().await)
}
async fn fail2ban_install(AuthUser(u): AuthUser) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(sk_security::fail2ban_install().await))
}
async fn fail2ban_jail(AuthUser(_): AuthUser, Path(jail): Path<String>) -> Json<Value> {
    Json(sk_security::fail2ban_jail(&jail).await)
}
async fn fail2ban_bans(AuthUser(_): AuthUser) -> Json<Value> {
    Json(sk_security::fail2ban_bans().await)
}
async fn fail2ban_ban(AuthUser(u): AuthUser, Json(b): Json<Value>) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(
        sk_security::fail2ban_ban(
            b["ip"].as_str().unwrap_or(""),
            b["jail"].as_str().unwrap_or("sshd"),
        )
        .await,
    ))
}
async fn fail2ban_unban(AuthUser(u): AuthUser, Json(b): Json<Value>) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(
        sk_security::fail2ban_unban(b["ip"].as_str().unwrap_or(""), b["jail"].as_str()).await,
    ))
}
#[derive(Deserialize)]
struct UserQ {
    user: Option<String>,
}
async fn ssh_keys(AuthUser(_): AuthUser, Query(q): Query<UserQ>) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_security::ssh_keys(q.user.as_deref().unwrap_or("root")).await?,
    ))
}
async fn ssh_add_key(AuthUser(u): AuthUser, Json(b): Json<Value>) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(
        sk_security::ssh_add_key(
            b["user"].as_str().unwrap_or("root"),
            b["key"].as_str().unwrap_or(""),
        )
        .await?,
    ))
}
async fn ssh_delete_key(
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
    Query(q): Query<UserQ>,
) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(
        sk_security::ssh_delete_key(q.user.as_deref().unwrap_or("root"), &id).await?,
    ))
}
async fn ip_lists(State(s): State<SharedState>, AuthUser(_): AuthUser) -> ApiResult<Json<Value>> {
    Ok(Json(sk_security::ip_lists(&s.db).await?))
}
async fn ip_list_add(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(list): Path<String>,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(
        sk_security::ip_list_add(
            &s.db,
            &list,
            b["ip"].as_str().unwrap_or(""),
            b["comment"].as_str(),
        )
        .await?,
    ))
}
async fn ip_list_delete(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path((list, ip)): Path<(String, String)>,
) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(sk_security::ip_list_delete(&s.db, &list, &ip).await?))
}
async fn lynis_status(AuthUser(_): AuthUser) -> Json<Value> {
    Json(sk_security::lynis_status().await)
}
async fn lynis_install(AuthUser(u): AuthUser) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(sk_security::lynis_install().await))
}
async fn lynis_scan(State(s): State<SharedState>, AuthUser(u): AuthUser) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(sk_security::lynis_scan(&s.db).await?))
}
async fn lynis_scan_status(AuthUser(_): AuthUser) -> Json<Value> {
    Json(json!({"status":"idle"}))
}
async fn auto_updates_status(AuthUser(_): AuthUser) -> Json<Value> {
    Json(sk_security::auto_updates_status().await)
}
async fn auto_updates_install(AuthUser(u): AuthUser) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(sk_security::auto_updates_install().await))
}
async fn auto_updates_enable(AuthUser(u): AuthUser) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(sk_security::auto_updates_enable().await))
}
async fn auto_updates_disable(AuthUser(u): AuthUser) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(sk_security::auto_updates_disable().await))
}

async fn waf_status(State(s): State<SharedState>, AuthUser(_): AuthUser) -> ApiResult<Json<Value>> {
    Ok(Json(sk_security::waf_status(&s.db).await?))
}
async fn waf_install(AuthUser(u): AuthUser) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(sk_security::waf_install().await))
}
async fn waf_policy(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(sk_security::waf_policy(&s.db, &id).await?))
}
async fn waf_set_policy(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(sk_security::waf_set_policy(&s.db, &id, &b).await?))
}
async fn waf_apply(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(sk_security::waf_apply(&s.db, &id).await?))
}
#[derive(Deserialize)]
struct WafQ {
    limit: Option<i64>,
}
async fn waf_events(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
    Query(q): Query<WafQ>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_security::waf_events(&s.db, &id, q.limit.unwrap_or(50)).await?,
    ))
}
