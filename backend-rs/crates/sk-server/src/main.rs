//! sk-server — ServerKit panel backend in Rust.
//!
//! P0 scope: auth (login/register/refresh/me/setup-status), Socket.IO
//! handshake (socketioxide — wire-compatible with the unmodified React
//! frontend), schema baseline, static frontend serving.

mod bootstrap;
mod db_backup;
mod error;
mod event_delivery;
mod extract;
mod le_renew;
mod monitor;
mod routes;
mod sampler;
mod sidecar;
mod socket;
mod state;

use axum::{routing::get, Router};
use sqlx::migrate::Migrator;
use state::AppState;
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tower_http::services::{ServeDir, ServeFile};
use tracing_subscriber::EnvFilter;

static MIGRATOR: Migrator = sqlx::migrate!("../../migrations");

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let config = sk_core::Config::from_env();
    tracing::info!(db = %config.database_path, port = config.port, "starting sk-server");

    let pool = sk_core::db::connect(&config.database_path).await?;
    sk_core::db::ensure_schema(&pool, &MIGRATOR).await?;
    // fork-owned tables + additive columns (safe to run every boot)
    sk_magento::store::ensure_schema(&pool).await?;
    sk_jobs::ensure_schema(&pool).await?;
    sk_projects::ensure_schema(&pool).await?;
    sk_events::ensure_schema(&pool).await?;
    sk_apps::ensure_schema(&pool).await?;
    sk_deploy::ensure_schema(&pool).await?;
    sk_runtimes::ensure_schema(&pool).await?;
    sk_backups::ensure_schema(&pool).await?;
    sk_dns::ensure_schema(&pool).await?;
    sk_web::domains::ensure_schema(&pool).await?;
    sk_web::ssl::ensure_schema(&pool).await?;
    sk_cloudflare::ensure_schema(&pool).await?;
    sk_security::ensure_schema(&pool).await?;
    sk_fleet::ensure_schema(&pool).await?;
    sk_cloud::ensure_schema(&pool).await?;
    sk_status::ensure_schema(&pool).await?;
    sk_ftp::ensure_schema(&pool).await?;
    sk_wordpress::ensure_schema(&pool).await?;
    sk_workflows::ensure_schema(&pool).await?;
    sk_git::ensure_schema(&pool).await?;
    // one-time: encrypt any plaintext store secrets left at rest
    sk_magento::store::encrypt_existing(&pool).await?;
    // optional non-interactive admin bootstrap (SK_BOOTSTRAP_ADMIN_*)
    bootstrap::run(&pool).await?;

    let (term_tx, term_rx) = tokio::sync::mpsc::unbounded_channel();
    let state = Arc::new(AppState {
        db: pool,
        config: config.clone(),
        terminal: sk_terminal::TerminalManager::new(),
        term_events: term_tx,
    });

    let (socket_layer, io) = socketioxide::SocketIo::new_layer();
    socket::register(&io, state.clone());
    socket::spawn_terminal_emitter(io.clone(), term_rx);
    // Boot the bundled AI sidecar (pi SDK) and wire SK_SIDECAR_URL/TOKEN.
    // Held for the process lifetime; killed on drop at shutdown.
    let _ai_sidecar = sidecar::autostart().await;

    sampler::spawn(state.db.clone());
    le_renew::spawn(state.db.clone());
    db_backup::spawn(state.db.clone());
    monitor::spawn(state.db.clone());
    event_delivery::spawn(state.db.clone());

    let api = Router::new()
        .route("/health", get(routes::health))
        .nest("/auth", routes::auth::router())
        .nest("/docker", routes::docker::router())
        .nest("/servers", routes::servers::router())
        .nest("/files", routes::files::router())
        .nest("/ai", routes::ai::router())
        .nest("/backups", routes::backups::router())
        .nest("/apps", routes::apps::router())
        .nest("/modules", routes::apps::modules_router())
        .nest("/image-updates", routes::apps::image_updates_router())
        .nest("/magento", routes::magento::router())
        .nest("/monitoring", routes::monitoring::router())
        .nest("/jobs", routes::jobs::jobs_router())
        .nest("/queue", routes::jobs::queue_router())
        .nest("/projects", routes::projects::projects_router())
        .nest("/environments", routes::projects::environments_router())
        .nest("/workspaces", routes::projects::workspaces_router())
        .nest("/api-keys", routes::projects::api_keys_router())
        .nest("/vaults", routes::projects::vaults_router())
        .nest("/secrets", routes::projects::secrets_router())
        .nest("/shared", routes::projects::shared_router())
        .nest("/telemetry", routes::events::telemetry_router())
        .nest("/notifications", routes::events::notifications_router())
        .nest(
            "/event-subscriptions",
            routes::events::event_subscriptions_router(),
        )
        .nest("/webhooks", routes::events::webhooks_router())
        .nest("/api-analytics", routes::events::api_analytics_router())
        .nest("/buildpacks", routes::deploy::buildpacks_router())
        .nest("/source-connections", routes::deploy::source_router())
        .nest("/connections", routes::deploy::connections_router())
        .nest("/deploy", routes::deploy::deploy_router())
        .nest("/builds", routes::deploy::builds_router())
        .nest("/deployment-jobs", routes::deploy::deployment_jobs_router())
        .nest("/migrations", routes::deploy::migrations_router())
        .nest("/node", routes::runtimes::node_router())
        .nest("/python", routes::runtimes::python_router())
        .merge(routes::templates::router())
        .route(
            "/dns/",
            get(routes::dns::zones).post(routes::dns::create_zone),
        )
        .nest("/dns", routes::dns::dns_router())
        .nest("/ddns", routes::dns::ddns_router())
        .nest("/registrars", routes::dns::registrars_router())
        .nest("/databases", routes::db::databases_router())
        .nest("/cron", routes::db::cron_router())
        .nest("/cloudflare", routes::cloudflare::router())
        .nest("/cloud", routes::cloud::router())
        .nest("/pairing", routes::fleet::pairing_router())
        .route(
            "/tunnels/",
            get(routes::fleet::tunnels).post(routes::fleet::create_tunnel),
        )
        .nest("/tunnels", routes::fleet::tunnels_router())
        .route(
            "/server-templates/",
            get(routes::fleet::templates).post(routes::fleet::create_template),
        )
        .nest("/server-templates", routes::fleet::templates_router())
        .nest("/fleet-monitor", routes::fleet::monitor_router())
        .route("/gpu", get(routes::gpu::info))
        .nest("/ftp", routes::ftp::router())
        .nest("/wordpress", routes::wordpress::router())
        .nest("/workflows", routes::workflows::router())
        .nest("/git", routes::git::router())
        .route(
            "/status/",
            get(routes::status::pages).post(routes::status::create_page),
        )
        .nest("/status", routes::status::status_router())
        .nest("/uptime", routes::status::uptime_router())
        .nest("/firewall", routes::security::firewall_router())
        .nest("/security", routes::security::security_router())
        .nest("/waf", routes::security::waf_router())
        .nest("/domains", routes::web::domains_router())
        .nest("/ssl", routes::web::ssl_router())
        .nest("/nginx", routes::web::nginx_router())
        .nest("/php", routes::web::php_router())
        .nest("/logs", routes::ops::logs_router())
        .nest("/processes", routes::ops::processes_router())
        .nest("/system", routes::system::router())
        .nest("/metrics", routes::system::metrics_router())
        .merge(routes::stubs::router())
        .merge(routes::compat::router())
        .with_state(state.clone())
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            routes::events::api_analytics_middleware,
        ));

    let index = format!("{}/index.html", config.frontend_dist);
    let spa = ServeDir::new(&config.frontend_dist).not_found_service(ServeFile::new(&index));

    let app = Router::new()
        .nest("/api/v1", api)
        .fallback_service(spa)
        .layer(socket_layer)
        .layer(CorsLayer::very_permissive());

    let addr = format!("0.0.0.0:{}", config.port);
    tracing::info!(%addr, "listening");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
