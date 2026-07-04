//! Socket.IO gateway (socketioxide 0.18 — wire-compatible with socket.io-client v4).
//!
//! Ports the relevant slice of `app/sockets.py`:
//! - connect: JWT auth (drop unauthenticated sockets), user context stored in
//!   socket extensions
//! - subscribe_terminal / unsubscribe_terminal: room join for PTY output
//!   (developer role required — viewers must not watch privileged shells)
//!
//! PTY output arrives from sk-terminal reader threads over an mpsc channel;
//! `spawn_terminal_emitter` rebroadcasts it as `server_stream` events, the
//! same shape the Flask agent gateway emits.

use crate::state::SharedState;
use serde_json::{json, Value};
use sk_auth::jwt::{decode_token, TokenType};
use socketioxide::extract::{SocketRef, TryData};
use socketioxide::SocketIo;
use tokio::sync::mpsc::UnboundedReceiver;

/// Authenticated user context attached to each socket.
#[derive(Debug, Clone)]
struct SocketUser {
    #[allow(dead_code)]
    user_id: i64,
    role: String,
}

impl SocketUser {
    /// `_client_is_privileged` — admin or developer.
    fn is_privileged(&self) -> bool {
        matches!(self.role.as_str(), "admin" | "developer")
    }
}

pub fn register(io: &SocketIo, state: SharedState) {
    io.ns(
        "/",
        async move |socket: SocketRef, TryData(auth): TryData<Value>| {
            let token = auth
                .ok()
                .as_ref()
                .and_then(|a| a.get("token"))
                .and_then(|t| t.as_str())
                .unwrap_or_default()
                .to_string();

            let claims = match decode_token(
                &token,
                &state.config.jwt_secret_key,
                TokenType::Access,
                false,
            ) {
                Ok(c) => c,
                Err(err) => {
                    tracing::warn!(sid = %socket.id, %err, "socket auth failed — disconnecting");
                    let _ = socket.disconnect();
                    return;
                }
            };
            let user_id = claims.sub.as_i64().unwrap_or(-1);

            // Role lookup once at connect (Flask does the same via _client_users).
            let role = sk_models::user::find_by_id(&state.db, user_id)
                .await
                .ok()
                .flatten()
                .map(|u| u.role().to_string())
                .unwrap_or_else(|| "viewer".into());

            tracing::info!(sid = %socket.id, user_id, %role, "socket connected");
            socket.extensions.insert(SocketUser { user_id, role });

            let term_state = state.clone();
            socket.on(
                "subscribe_terminal",
                async move |socket: SocketRef, TryData(data): TryData<Value>| {
                    let user: Option<SocketUser> = socket.extensions.get::<SocketUser>();
                    if !user.map(|u| u.is_privileged()).unwrap_or(false) {
                        let _ = socket.emit(
                            "error",
                            &json!({ "message": "Developer role required for terminal access" }),
                        );
                        return;
                    }
                    let Some(session_id) = data
                        .ok()
                        .as_ref()
                        .and_then(|d| d.get("session_id"))
                        .and_then(|s| s.as_str())
                        .map(str::to_string)
                    else {
                        let _ = socket.emit("error", &json!({ "message": "session_id required" }));
                        return;
                    };
                    let Some(room) = term_state.terminal.session_room(&session_id) else {
                        let _ =
                            socket.emit("error", &json!({ "message": "Unknown terminal session" }));
                        return;
                    };
                    socket.join(room);
                    let _ = socket.emit(
                        "subscribed",
                        &json!({ "channel": format!("terminal:{session_id}") }),
                    );
                },
            );

            let unsub_state = state.clone();
            socket.on(
                "unsubscribe_terminal",
                async move |socket: SocketRef, TryData(data): TryData<Value>| {
                    let Some(session_id) = data
                        .ok()
                        .as_ref()
                        .and_then(|d| d.get("session_id"))
                        .and_then(|s| s.as_str())
                        .map(str::to_string)
                    else {
                        return;
                    };
                    if let Some(room) = unsub_state.terminal.session_room(&session_id) {
                        socket.leave(room);
                    }
                },
            );

            socket.on_disconnect(async |s: SocketRef| {
                tracing::debug!(sid = %s.id, "socket disconnected");
            });
        },
    );
}

/// Rebroadcast PTY output as `server_stream` events — the exact shape the
/// Flask agent gateway emits (`agent_gateway.py`).
pub fn spawn_terminal_emitter(io: SocketIo, mut rx: UnboundedReceiver<sk_terminal::TermEvent>) {
    tokio::spawn(async move {
        while let Some(ev) = rx.recv().await {
            let (session_id, payload) = match ev {
                sk_terminal::TermEvent::Output {
                    session_id,
                    data_b64,
                } => (
                    session_id.clone(),
                    json!({ "type": "output", "data": data_b64 }),
                ),
                sk_terminal::TermEvent::Closed { session_id } => {
                    (session_id.clone(), json!({ "type": "closed" }))
                }
            };
            let room = sk_terminal::room_for("local", &session_id);
            let msg = json!({
                "server_id": "local",
                "channel": format!("terminal:{session_id}"),
                "data": payload,
            });
            if let Err(e) = io.to(room).emit("server_stream", &msg).await {
                tracing::debug!(error = %e, "terminal stream emit failed");
            }
        }
    });
}
