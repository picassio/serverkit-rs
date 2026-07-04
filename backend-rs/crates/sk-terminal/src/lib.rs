//! sk-terminal — local PTY terminal sessions.
//!
//! Wire-compatible with ServerKit's agent terminal contract
//! (`TerminalService` + agent gateway): sessions are created over REST,
//! keystrokes arrive as base64 REST posts, and PTY output is streamed as
//! base64 `server_stream` Socket.IO events on channel `terminal:<sid>`
//! into room `server_<server_id>_terminal:<sid>`.
//!
//! DIVERGENCE (improvement): upstream Flask refuses `server_id == 'local'`
//! (terminals only exist on remote agents). This backend runs a real local
//! PTY via portable-pty, so the panel host gets a first-class terminal.

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc::UnboundedSender;

/// Events forwarded from PTY reader threads to the Socket.IO emitter task.
#[derive(Debug)]
pub enum TermEvent {
    Output {
        session_id: String,
        data_b64: String,
    },
    Closed {
        session_id: String,
    },
}

struct Session {
    server_id: String,
    user_id: i64,
    shell: String,
    cols: u16,
    rows: u16,
    created_at: String,
    writer: Mutex<Box<dyn Write + Send>>,
    master: Mutex<Box<dyn MasterPty + Send>>,
    child: Mutex<Box<dyn Child + Send + Sync>>,
}

#[derive(Clone, Default)]
pub struct TerminalManager {
    sessions: Arc<Mutex<HashMap<String, Arc<Session>>>>,
}

impl TerminalManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// `TerminalService.create_session` (local variant).
    pub fn create_session(
        &self,
        user_id: i64,
        cols: u16,
        rows: u16,
        events: UnboundedSender<TermEvent>,
    ) -> Value {
        let session_id = format!("term_{}", &uuid::Uuid::new_v4().simple().to_string()[..12]);
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".into());

        let pty = native_pty_system();
        let pair = match pty.openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        }) {
            Ok(p) => p,
            Err(e) => return json!({ "success": false, "error": e.to_string() }),
        };

        let mut cmd = CommandBuilder::new(&shell);
        cmd.env("TERM", "xterm-256color");
        if let Ok(home) = std::env::var("HOME") {
            cmd.cwd(home);
        }

        let child = match pair.slave.spawn_command(cmd) {
            Ok(c) => c,
            Err(e) => return json!({ "success": false, "error": e.to_string() }),
        };
        drop(pair.slave);

        let mut reader = match pair.master.try_clone_reader() {
            Ok(r) => r,
            Err(e) => return json!({ "success": false, "error": e.to_string() }),
        };
        let writer = match pair.master.take_writer() {
            Ok(w) => w,
            Err(e) => return json!({ "success": false, "error": e.to_string() }),
        };

        let session = Arc::new(Session {
            server_id: "local".to_string(),
            user_id,
            shell: shell.clone(),
            cols,
            rows,
            created_at: chrono::Utc::now()
                .naive_utc()
                .format("%Y-%m-%dT%H:%M:%S%.6f")
                .to_string(),
            writer: Mutex::new(writer),
            master: Mutex::new(pair.master),
            child: Mutex::new(child),
        });

        self.sessions
            .lock()
            .unwrap()
            .insert(session_id.clone(), session);

        // Blocking PTY reader on a plain thread; output crosses into async
        // land via the unbounded channel.
        let sid = session_id.clone();
        let sessions = self.sessions.clone();
        std::thread::spawn(move || {
            let mut buf = [0u8; 8192];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        let ev = TermEvent::Output {
                            session_id: sid.clone(),
                            data_b64: B64.encode(&buf[..n]),
                        };
                        if events.send(ev).is_err() {
                            break;
                        }
                    }
                }
            }
            let _ = events.send(TermEvent::Closed {
                session_id: sid.clone(),
            });
            sessions.lock().unwrap().remove(&sid);
            tracing::info!(session_id = %sid, "terminal session ended");
        });

        json!({
            "success": true,
            "session_id": session_id,
            "server_id": "local",
            "shell": shell,
            "cols": cols,
            "rows": rows
        })
    }

    /// `TerminalService.send_input` — `data` is base64.
    pub fn send_input(&self, session_id: &str, user_id: i64, data_b64: &str) -> Value {
        let Some(session) = self.get(session_id) else {
            return json!({ "success": false, "error": "Session not found" });
        };
        if session.user_id != user_id {
            return json!({ "success": false, "error": "Unauthorized" });
        }
        let bytes = match B64.decode(data_b64) {
            Ok(b) => b,
            Err(e) => return json!({ "success": false, "error": e.to_string() }),
        };
        let mut writer = session.writer.lock().unwrap();
        match writer.write_all(&bytes).and_then(|_| writer.flush()) {
            Ok(_) => json!({ "success": true }),
            Err(e) => json!({ "success": false, "error": e.to_string() }),
        }
    }

    /// `TerminalService.resize_session`
    pub fn resize(&self, session_id: &str, user_id: i64, cols: u16, rows: u16) -> Value {
        let Some(session) = self.get(session_id) else {
            return json!({ "success": false, "error": "Session not found" });
        };
        if session.user_id != user_id {
            return json!({ "success": false, "error": "Unauthorized" });
        }
        let result = session.master.lock().unwrap().resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        });
        match result {
            Ok(_) => json!({ "success": true }),
            Err(e) => json!({ "success": false, "error": e.to_string() }),
        }
    }

    /// `TerminalService.close_session`
    pub fn close(&self, session_id: &str, user_id: i64) -> Value {
        let Some(session) = self.get(session_id) else {
            return json!({ "success": false, "error": "Session not found" });
        };
        if session.user_id != user_id {
            return json!({ "success": false, "error": "Unauthorized" });
        }
        if let Err(e) = session.child.lock().unwrap().kill() {
            tracing::warn!(session_id, error = %e, "failed to kill terminal child");
        }
        self.sessions.lock().unwrap().remove(session_id);
        json!({ "success": true })
    }

    /// `TerminalService.get_user_sessions`
    pub fn user_sessions(&self, user_id: i64) -> Vec<Value> {
        self.sessions
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, s)| s.user_id == user_id)
            .map(|(id, s)| {
                json!({
                    "session_id": id,
                    "server_id": s.server_id,
                    "shell": s.shell,
                    "cols": s.cols,
                    "rows": s.rows,
                    "created_at": s.created_at,
                })
            })
            .collect()
    }

    /// Session metadata for socket room subscription (`get_session`).
    pub fn session_room(&self, session_id: &str) -> Option<String> {
        self.get(session_id)
            .map(|s| format!("server_{}_terminal:{}", s.server_id, session_id))
    }

    fn get(&self, session_id: &str) -> Option<Arc<Session>> {
        self.sessions.lock().unwrap().get(session_id).cloned()
    }
}

/// Room name for a session's output stream.
pub fn room_for(server_id: &str, session_id: &str) -> String {
    format!("server_{server_id}_terminal:{session_id}")
}
