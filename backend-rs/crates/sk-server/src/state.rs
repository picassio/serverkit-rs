use sqlx::SqlitePool;
use tokio::sync::mpsc::UnboundedSender;

pub struct AppState {
    pub db: SqlitePool,
    pub config: sk_core::Config,
    pub terminal: sk_terminal::TerminalManager,
    /// PTY output events, drained by the Socket.IO emitter task in main.
    pub term_events: UnboundedSender<sk_terminal::TermEvent>,
}

pub type SharedState = std::sync::Arc<AppState>;
