//! AI assistant - backed by the `pi` CLI (the pi SDK) as a sidecar. The
//! Rust backend spawns `pi -p --mode json`, maps its NDJSON `message_update`
//! events to the SSE contract the frontend expects (`open`/`text_delta`/
//! `done`), and persists conversations in ai_conversations/ai_messages.
//!
//! Ported endpoints from `app/api/ai.py`: status, settings, providers,
//! models, tools, conversations CRUD, chat/stream. Runs `--no-tools`
//! (answers/guidance; no destructive execution) for safety.

use crate::error::{ApiError, ApiResult};
use crate::extract::AuthUser;
use crate::state::SharedState;
use axum::extract::{Path, State};
use axum::response::sse::{Event, Sse};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use std::convert::Infallible;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};

pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/status", get(status))
        .route("/settings", get(get_settings).put(put_settings))
        .route("/providers", get(providers))
        .route("/models", get(models))
        .route("/tools", get(tools))
        .route(
            "/conversations",
            get(list_conversations).post(create_conversation),
        )
        .route(
            "/conversations/{id}",
            get(get_conversation)
                .patch(patch_conversation)
                .delete(delete_conversation),
        )
        .route("/chat/stream", post(chat_stream))
        .route("/chat", post(chat))
        .route("/auth/status", get(auth_status))
        .route("/auth/login/start", post(auth_login_start))
        .route("/auth/login/complete", post(auth_login_complete))
        .route("/auth/logout", post(auth_logout))
}

fn require_admin(u: &sk_models::user::User) -> ApiResult<()> {
    if !u.is_admin() {
        return Err(ApiError::forbidden("Admin access required"));
    }
    Ok(())
}

// ── config (SK_DATA_DIR/ai.json) ────────────────────────────────────────
fn ai_config_path() -> std::path::PathBuf {
    std::path::PathBuf::from(std::env::var("SK_DATA_DIR").unwrap_or_else(|_| "data".into()))
        .join("ai.json")
}
fn ai_config() -> Value {
    std::fs::read_to_string(ai_config_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_else(
            || json!({ "enabled": true, "provider": "", "model": "", "tools_enabled": true }),
        )
}

/// Path to the serverkit-tools pi extension, if configured + present.
fn tools_extension() -> Option<String> {
    let p = std::env::var("SK_TOOLS_EXTENSION").ok()?;
    std::path::Path::new(&p).exists().then_some(p)
}

/// Whether the agent should get native tools this turn (mode != simple and
/// tools_enabled in config). Applies to both the CLI and sidecar paths.
fn agent_tools_enabled(mode: &str) -> bool {
    mode != "simple" && ai_config()["tools_enabled"].as_bool().unwrap_or(true)
}

/// Extension path for the CLI path only (sidecar registers tools in-process).
fn agent_tools(mode: &str) -> Option<String> {
    if agent_tools_enabled(mode) {
        tools_extension()
    } else {
        None
    }
}

/// Full-SDK sidecar base URL (e.g. http://127.0.0.1:5056), if configured.
fn sidecar_url() -> Option<String> {
    std::env::var("SK_SIDECAR_URL")
        .ok()
        .filter(|s| !s.is_empty())
}

/// Proxy one turn to the pi-SDK sidecar, forwarding its SSE into `tx` and
/// returning the accumulated assistant text (for persistence). Rust owns the
/// open/done envelope, so those sidecar events are dropped here.
async fn proxy_sidecar(
    base: &str,
    conv: &str,
    message: &str,
    tools_enabled: bool,
    api_url: &str,
    api_token: &str,
    model: &str,
    tx: &tokio::sync::mpsc::Sender<Result<Event, Infallible>>,
) -> Result<String, String> {
    use futures_util::StreamExt;
    let mut body = json!({
        "conversation_id": conv, "message": message,
        "tools_enabled": tools_enabled, "api_url": api_url, "api_token": api_token,
    });
    if !model.is_empty() {
        body["model"] = json!(model);
    }
    let mut req = reqwest::Client::new()
        .post(format!("{}/chat/stream", base.trim_end_matches('/')))
        .json(&body);
    if let Ok(sec) = std::env::var("SK_SIDECAR_TOKEN") {
        if !sec.is_empty() {
            req = req.header("x-sk-sidecar-token", sec);
        }
    }
    let resp = req.send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("sidecar HTTP {}", resp.status()));
    }
    let mut stream = resp.bytes_stream();
    let mut buf = String::new();
    let mut acc = String::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| e.to_string())?;
        buf.push_str(&String::from_utf8_lossy(&chunk));
        while let Some(pos) = buf.find("\n\n") {
            let record: String = buf.drain(..pos + 2).collect();
            let mut ev_name = String::new();
            let mut data = String::new();
            for line in record.lines() {
                if let Some(v) = line.strip_prefix("event:") {
                    ev_name = v.trim().to_string();
                } else if let Some(v) = line.strip_prefix("data:") {
                    if !data.is_empty() {
                        data.push('\n');
                    }
                    data.push_str(v.trim());
                }
            }
            match ev_name.as_str() {
                "" | "open" | "done" => {}
                "text_delta" => {
                    if let Ok(d) = serde_json::from_str::<Value>(&data) {
                        if let Some(t) = d["text"].as_str() {
                            acc.push_str(t);
                        }
                    }
                    let _ = tx
                        .send(Ok(Event::default().event("text_delta").data(data)))
                        .await;
                }
                other => {
                    let _ = tx
                        .send(Ok(Event::default().event(other.to_string()).data(data)))
                        .await;
                }
            }
        }
    }
    Ok(acc)
}
fn save_ai_config(c: &Value) {
    let p = ai_config_path();
    if let Some(d) = p.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let _ = std::fs::write(p, serde_json::to_string_pretty(c).unwrap_or_default());
}
fn pi_available() -> bool {
    std::process::Command::new("pi")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
fn auth_present() -> bool {
    std::env::var("HOME")
        .map(|h| {
            std::path::Path::new(&h)
                .join(".pi/agent/auth.json")
                .exists()
        })
        .unwrap_or(false)
}
fn sessions_dir() -> String {
    std::path::PathBuf::from(std::env::var("SK_DATA_DIR").unwrap_or_else(|_| "data".into()))
        .join("ai-sessions")
        .to_string_lossy()
        .into_owned()
}

const SYSTEM_PROMPT: &str =
    "You are the ServerKit assistant, embedded in a server control panel that \
manages web apps, databases, Docker, nginx, PHP, and Magento stores. Answer concisely and \
practically. When ServerKit tools (names prefixed sk_) are available, use them to read live \
state and perform the operator's requested actions (create/manage Magento stores and websites, \
run Magento actions, control containers, back up databases, install templates). Prefer a read \
tool to confirm state before and after a write. Be careful with destructive actions and confirm \
intent in your reply. When no tools are available, give guidance the operator can act on.";

// ── status / settings ───────────────────────────────────────────────────
async fn status(AuthUser(_u): AuthUser) -> Json<Value> {
    let cfg = ai_config();
    let configured = pi_available() && auth_present();
    Json(json!({
        "enabled": cfg["enabled"].as_bool().unwrap_or(true) && configured,
        "configured": configured,
        "provider": cfg["provider"],
        "model": cfg["model"],
        "tools_count": if sidecar_url().is_some() || agent_tools("assistant").is_some() { 18 } else { 0 },
        "backend": if sidecar_url().is_some() { "sdk-sidecar" } else { "pi-cli" },
        "mode_default": "assistant",
        "pii_redaction": false,
        "injection_detection": false,
    }))
}

async fn get_settings(AuthUser(u): AuthUser) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    let cfg = ai_config();
    Ok(Json(json!({
        "enabled": cfg["enabled"].as_bool().unwrap_or(true),
        "provider": cfg["provider"],
        "model": cfg["model"],
        "endpoint": "",
        "api_key_set": auth_present(),  // pi manages its own auth.json
    })))
}

async fn put_settings(AuthUser(u): AuthUser, Json(data): Json<Value>) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    let mut cfg = ai_config();
    for k in ["enabled", "provider", "model"] {
        if let Some(v) = data.get(k) {
            cfg[k] = v.clone();
        }
    }
    save_ai_config(&cfg);
    Ok(Json(
        json!({ "ok": true, "configured": pi_available() && auth_present() }),
    ))
}

async fn providers(AuthUser(u): AuthUser) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    // pi manages providers/keys itself; expose the common ones for the picker.
    Ok(Json(
        json!({ "providers": ["anthropic", "google", "openai"] }),
    ))
}
async fn models(AuthUser(u): AuthUser) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(json!({ "models": [] })))
}
async fn tools(AuthUser(u): AuthUser) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(json!({ "tools": [] })))
}

// ── conversations ───────────────────────────────────────────────────────
async fn list_conversations(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
) -> ApiResult<Json<Value>> {
    let rows: Vec<(String, Option<String>, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT id, title, mode, updated_at FROM ai_conversations WHERE user_id = ? ORDER BY updated_at DESC LIMIT 100",
    )
    .bind(u.id)
    .fetch_all(&s.db)
    .await
    .map_err(anyhow::Error::from)?;
    let conversations: Vec<Value> = rows
        .into_iter()
        .map(|(id, title, mode, updated)| json!({ "id": id, "title": title, "mode": mode, "updated_at": updated }))
        .collect();
    Ok(Json(json!({ "conversations": conversations })))
}

#[derive(Deserialize, Default)]
struct CreateConvBody {
    mode: Option<String>,
    title: Option<String>,
}
async fn create_conversation(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    body: Option<Json<CreateConvBody>>,
) -> ApiResult<Json<Value>> {
    let b = body.map(|b| b.0).unwrap_or_default();
    let id = uuid_v4();
    let now = sk_core::time::now_sql();
    sqlx::query("INSERT INTO ai_conversations (id, user_id, title, mode, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)")
        .bind(&id).bind(u.id).bind(b.title).bind(b.mode.unwrap_or_else(|| "assistant".into())).bind(&now).bind(&now)
        .execute(&s.db).await.map_err(anyhow::Error::from)?;
    Ok(Json(json!({ "conversation": { "id": id } })))
}

async fn get_conversation(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    let conv: Option<(String, Option<String>, Option<String>)> =
        sqlx::query_as("SELECT id, title, mode FROM ai_conversations WHERE id = ? AND user_id = ?")
            .bind(&id)
            .bind(u.id)
            .fetch_optional(&s.db)
            .await
            .map_err(anyhow::Error::from)?;
    let Some((id, title, mode)) = conv else {
        return Err(ApiError::not_found("Conversation not found"));
    };
    let msgs: Vec<(String, Option<String>, Option<String>)> =
        sqlx::query_as("SELECT role, content, created_at FROM ai_messages WHERE conversation_id = ? ORDER BY id ASC")
            .bind(&id).fetch_all(&s.db).await.map_err(anyhow::Error::from)?;
    let messages: Vec<Value> = msgs
        .into_iter()
        .map(|(role, content, at)| json!({ "role": role, "content": content, "created_at": at }))
        .collect();
    Ok(Json(
        json!({ "conversation": { "id": id, "title": title, "mode": mode, "messages": messages } }),
    ))
}

#[derive(Deserialize)]
struct PatchConvBody {
    title: Option<String>,
}
async fn patch_conversation(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
    Json(b): Json<PatchConvBody>,
) -> ApiResult<Json<Value>> {
    sqlx::query(
        "UPDATE ai_conversations SET title = ?, updated_at = ? WHERE id = ? AND user_id = ?",
    )
    .bind(b.title)
    .bind(sk_core::time::now_sql())
    .bind(&id)
    .bind(u.id)
    .execute(&s.db)
    .await
    .map_err(anyhow::Error::from)?;
    Ok(Json(json!({ "ok": true })))
}
async fn delete_conversation(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    sqlx::query("DELETE FROM ai_messages WHERE conversation_id = ?")
        .bind(&id)
        .execute(&s.db)
        .await
        .ok();
    sqlx::query("DELETE FROM ai_conversations WHERE id = ? AND user_id = ?")
        .bind(&id)
        .bind(u.id)
        .execute(&s.db)
        .await
        .map_err(anyhow::Error::from)?;
    Ok(Json(json!({ "ok": true })))
}

// ── chat ────────────────────────────────────────────────────────────────
#[derive(Deserialize)]
struct ChatBody {
    conversation_id: Option<String>,
    message: Option<String>,
    #[serde(default)]
    mode: Option<String>,
    #[serde(default)]
    page_context: Value,
    #[serde(default)]
    model: Option<String>,
}

// ── provider auth: thin admin-only proxy to the sidecar's /auth/* ──────
async fn sidecar_call(
    method: reqwest::Method,
    path: &str,
    body: Option<Value>,
) -> ApiResult<Json<Value>> {
    let base = sidecar_url().ok_or_else(|| {
        ApiError::new(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "AI sidecar is not configured",
        )
    })?;
    let mut req =
        reqwest::Client::new().request(method, format!("{}{}", base.trim_end_matches('/'), path));
    if let Ok(sec) = std::env::var("SK_SIDECAR_TOKEN") {
        if !sec.is_empty() {
            req = req.header("x-sk-sidecar-token", sec);
        }
    }
    if let Some(b) = body {
        req = req.json(&b);
    }
    let resp = req.send().await.map_err(|e| {
        ApiError::new(
            axum::http::StatusCode::BAD_GATEWAY,
            format!("sidecar unreachable: {e}"),
        )
    })?;
    let status = resp.status();
    let val: Value = resp.json().await.unwrap_or(json!({}));
    if !status.is_success() {
        let msg = val["error"].as_str().unwrap_or("sidecar error").to_string();
        return Err(ApiError::new(
            axum::http::StatusCode::from_u16(status.as_u16())
                .unwrap_or(axum::http::StatusCode::BAD_GATEWAY),
            msg,
        ));
    }
    Ok(Json(val))
}

async fn auth_status(AuthUser(u): AuthUser) -> ApiResult<Json<Value>> {
    if !u.is_admin() {
        return Err(ApiError::forbidden("Admin access required"));
    }
    sidecar_call(reqwest::Method::GET, "/auth/status", None).await
}
async fn auth_login_start(AuthUser(u): AuthUser, Json(b): Json<Value>) -> ApiResult<Json<Value>> {
    if !u.is_admin() {
        return Err(ApiError::forbidden("Admin access required"));
    }
    sidecar_call(reqwest::Method::POST, "/auth/login/start", Some(b)).await
}
async fn auth_login_complete(
    AuthUser(u): AuthUser,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    if !u.is_admin() {
        return Err(ApiError::forbidden("Admin access required"));
    }
    sidecar_call(reqwest::Method::POST, "/auth/login/complete", Some(b)).await
}
async fn auth_logout(AuthUser(u): AuthUser, Json(b): Json<Value>) -> ApiResult<Json<Value>> {
    if !u.is_admin() {
        return Err(ApiError::forbidden("Admin access required"));
    }
    sidecar_call(reqwest::Method::POST, "/auth/logout", Some(b)).await
}

async fn ensure_conversation(
    s: &SharedState,
    user_id: i64,
    conv_id: &Option<String>,
    mode: &str,
    first_msg: &str,
) -> Result<String, ApiError> {
    if let Some(id) = conv_id {
        let exists: Option<(String,)> =
            sqlx::query_as("SELECT id FROM ai_conversations WHERE id = ? AND user_id = ?")
                .bind(id)
                .bind(user_id)
                .fetch_optional(&s.db)
                .await
                .map_err(anyhow::Error::from)?;
        if exists.is_some() {
            return Ok(id.clone());
        }
    }
    let id = uuid_v4();
    let title: String = first_msg.chars().take(60).collect();
    let now = sk_core::time::now_sql();
    sqlx::query("INSERT INTO ai_conversations (id, user_id, title, mode, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)")
        .bind(&id).bind(user_id).bind(title).bind(mode).bind(&now).bind(&now)
        .execute(&s.db).await.map_err(anyhow::Error::from)?;
    Ok(id)
}

async fn persist_message(s: &SharedState, conv_id: &str, role: &str, content: &str) {
    let now = sk_core::time::now_sql();
    let _ = sqlx::query(
        "INSERT INTO ai_messages (conversation_id, role, content, created_at) VALUES (?, ?, ?, ?)",
    )
    .bind(conv_id)
    .bind(role)
    .bind(content)
    .bind(&now)
    .execute(&s.db)
    .await;
    let _ = sqlx::query("UPDATE ai_conversations SET updated_at = ? WHERE id = ?")
        .bind(&now)
        .bind(conv_id)
        .execute(&s.db)
        .await;
}

/// POST /ai/chat/stream — SSE. Spawns pi, streams text_delta diffs.
async fn chat_stream(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Json(b): Json<ChatBody>,
) -> ApiResult<impl IntoResponse> {
    if !(pi_available() && auth_present()) {
        return Err(ApiError::new(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "AI assistant is not configured",
        ));
    }
    let message = b
        .message
        .map(|m| m.trim().to_string())
        .filter(|m| !m.is_empty())
        .ok_or_else(|| ApiError::bad_request("message is required"))?;
    let mode = b.mode.unwrap_or_else(|| "assistant".into());
    let conv_id = ensure_conversation(&s, u.id, &b.conversation_id, &mode, &message).await?;
    persist_message(&s, &conv_id, "user", &message).await;

    // Agent-mode tool wiring (per-user short-lived token + extension path).
    let tools_enabled = agent_tools_enabled(&mode);
    let tools_ext = agent_tools(&mode);
    let api_url = format!("http://127.0.0.1:{}", s.config.port);
    let api_token = if tools_enabled {
        sk_auth::jwt::create_token(
            u.id,
            sk_auth::jwt::TokenType::Access,
            3600,
            &s.config.jwt_secret_key,
            false,
        )
        .unwrap_or_default()
    } else {
        String::new()
    };

    // Model: request override, else configured default (empty = sidecar default).
    let model = b
        .model
        .clone()
        .filter(|m| !m.is_empty())
        .or_else(|| {
            ai_config()["model"]
                .as_str()
                .filter(|x| !x.is_empty())
                .map(String::from)
        })
        .unwrap_or_default();

    let (tx, rx) = tokio::sync::mpsc::channel::<Result<Event, Infallible>>(64);
    let db = s.clone();
    let conv = conv_id.clone();
    let context_str = context_prompt(&b.page_context);

    tokio::spawn(async move {
        let _ = tx
            .send(Ok(Event::default()
                .event("open")
                .data(json!({ "conversation_id": conv }).to_string())))
            .await;

        // Full-SDK path: proxy to the pi-SDK sidecar when configured.
        if let Some(base) = sidecar_url() {
            match proxy_sidecar(
                &base,
                &conv,
                &message,
                tools_enabled,
                &api_url,
                &api_token,
                &model,
                &tx,
            )
            .await
            {
                Ok(acc) => {
                    if !acc.is_empty() {
                        persist_message(&db, &conv, "assistant", &acc).await;
                    }
                }
                Err(e) => {
                    let _ = tx
                        .send(Ok(Event::default()
                            .event("error")
                            .data(json!({ "message": e }).to_string())))
                        .await;
                }
            }
            let _ = tx
                .send(Ok(Event::default()
                    .event("done")
                    .data(json!({ "conversation_id": conv }).to_string())))
                .await;
            return;
        }

        // Native-CLI fallback path.
        let mut cmd = tokio::process::Command::new("pi");
        cmd.args([
            "-p",
            "--mode",
            "json",
            "--session-dir",
            &sessions_dir(),
            "--session-id",
            &conv,
        ]);
        // Agent mode: load serverkit-tools + inject a per-user API token so the
        // agent acts AS the user (RBAC enforced server-side). Else --no-tools.
        match &tools_ext {
            Some(ext) => {
                cmd.args(["-e", ext]);
                cmd.env("SK_API_URL", &api_url);
                cmd.env("SK_API_TOKEN", &api_token);
            }
            None => {
                cmd.arg("--no-tools");
            }
        }
        let cfg = ai_config();
        if let Some(p) = cfg["provider"].as_str().filter(|x| !x.is_empty()) {
            cmd.args(["--provider", p]);
        }
        if let Some(m) = cfg["model"].as_str().filter(|x| !x.is_empty()) {
            cmd.args(["--model", m]);
        }
        cmd.args(["--append-system-prompt", SYSTEM_PROMPT]);
        if !context_str.is_empty() {
            cmd.args(["--append-system-prompt", &context_str]);
        }
        cmd.arg(&message);
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                let _ = tx
                    .send(Ok(Event::default()
                        .event("error")
                        .data(json!({ "message": e.to_string() }).to_string())))
                    .await;
                return;
            }
        };
        let stdout = child.stdout.take().unwrap();
        let mut lines = BufReader::new(stdout).lines();
        let mut acc = String::new();

        while let Ok(Some(line)) = lines.next_line().await {
            let Ok(ev) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            // tool activity -> the frontend's existing tool SSE events
            if ev["type"] == "tool_execution_start" {
                let id = ev["toolCallId"].as_str().unwrap_or("");
                let name = ev["toolName"].as_str().unwrap_or("");
                let _ = tx
                    .send(Ok(Event::default()
                        .event("tool_use_start")
                        .data(json!({ "id": id, "name": name }).to_string())))
                    .await;
                let _ = tx
                    .send(Ok(Event::default().event("tool_use_stop").data(
                        json!({ "id": id, "name": name, "input": ev["args"] }).to_string(),
                    )))
                    .await;
                continue;
            }
            if ev["type"] == "tool_execution_end" {
                let id = ev["toolCallId"].as_str().unwrap_or("");
                let output: String = ev["result"]["content"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|c| c["text"].as_str())
                    .collect();
                let is_err = ev["result"]["isError"].as_bool().unwrap_or(false);
                let _ = tx
                    .send(Ok(Event::default().event("tool_result").data(
                        json!({ "id": id, "output": output, "is_error": is_err }).to_string(),
                    )))
                    .await;
                continue;
            }
            if ev["type"] == "message_update" && ev["message"]["role"] == "assistant" {
                let full: String = ev["message"]["content"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter(|c| c["type"] == "text")
                    .filter_map(|c| c["text"].as_str())
                    .collect();
                if full.len() > acc.len() && full.starts_with(&acc) {
                    let delta = &full[acc.len()..];
                    let _ = tx
                        .send(Ok(Event::default()
                            .event("text_delta")
                            .data(json!({ "text": delta }).to_string())))
                        .await;
                    acc = full;
                } else if full != acc {
                    // non-prefix change: resend full (rare)
                    let _ = tx
                        .send(Ok(Event::default()
                            .event("text_delta")
                            .data(json!({ "text": &full }).to_string())))
                        .await;
                    acc = full;
                }
            }
        }
        let _ = child.wait().await;
        if !acc.is_empty() {
            persist_message(&db, &conv, "assistant", &acc).await;
        }
        let _ = tx
            .send(Ok(Event::default()
                .event("done")
                .data(json!({ "conversation_id": conv }).to_string())))
            .await;
    });

    Ok(Sse::new(tokio_stream::wrappers::ReceiverStream::new(rx)))
}

/// POST /ai/chat — non-streaming (runs pi to completion).
async fn chat(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Json(b): Json<ChatBody>,
) -> ApiResult<Json<Value>> {
    if !(pi_available() && auth_present()) {
        return Err(ApiError::new(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "AI assistant is not configured",
        ));
    }
    let message = b
        .message
        .map(|m| m.trim().to_string())
        .filter(|m| !m.is_empty())
        .ok_or_else(|| ApiError::bad_request("message is required"))?;
    let mode = b.mode.unwrap_or_else(|| "assistant".into());
    let conv_id = ensure_conversation(&s, u.id, &b.conversation_id, &mode, &message).await?;
    persist_message(&s, &conv_id, "user", &message).await;

    let mut cmd = tokio::process::Command::new("pi");
    cmd.args([
        "-p",
        "--mode",
        "json",
        "--no-tools",
        "--session-dir",
        &sessions_dir(),
        "--session-id",
        &conv_id,
        "--append-system-prompt",
        SYSTEM_PROMPT,
    ])
    .arg(&message)
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());
    let out = cmd
        .output()
        .await
        .map_err(|e| ApiError::new(axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let mut reply = String::new();
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        if let Ok(ev) = serde_json::from_str::<Value>(line) {
            if ev["type"] == "message_end" && ev["message"]["role"] == "assistant" {
                reply = ev["message"]["content"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter(|c| c["type"] == "text")
                    .filter_map(|c| c["text"].as_str())
                    .collect();
            }
        }
    }
    persist_message(&s, &conv_id, "assistant", &reply).await;
    Ok(Json(json!({ "conversation_id": conv_id, "reply": reply })))
}

fn context_prompt(page_context: &Value) -> String {
    if !page_context.is_object()
        || page_context
            .as_object()
            .map(|m| m.is_empty())
            .unwrap_or(true)
    {
        return String::new();
    }
    format!("Current panel context (JSON): {}", page_context)
}

fn uuid_v4() -> String {
    // lightweight uuid without a dep collision: use sk-auth's uuid via random hex
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let b: [u8; 16] = rng.gen();
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-4{:01x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        b[0], b[1], b[2], b[3], b[4], b[5], b[6] & 0x0f, b[7], (b[8] & 0x3f) | 0x80, b[9],
        b[10], b[11], b[12], b[13], b[14], b[15]
    )
}
