mod app_state;
mod auth;
mod db;
mod handlers;
mod models;
mod nginx_ops;

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use app_state::{AppConfig, AppState};
use axum::Router;
use axum::routing::{get, post};
use tokio::sync::RwLock;
use tracing::info;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "nginx_manager=info,info".to_string()),
        )
        .init();

    let cfg = AppConfig::from_env();
    let pool = db::connect(&cfg.database_url).await?;
    db::init(&pool).await?;

    let state = AppState {
        cfg: cfg.clone(),
        pool,
        sessions: Arc::new(RwLock::new(HashMap::new())),
    };

    let app = Router::new()
        .route("/", get(handlers::dashboard))
        .route("/login", get(handlers::login_page).post(handlers::login))
        .route("/logout", post(handlers::logout))
        .route(
            "/sites",
            get(handlers::list_sites).post(handlers::create_site),
        )
        .route("/sites/:id/edit", get(handlers::edit_site_page))
        .route("/sites/:id/update", post(handlers::update_site))
        .route("/sites/:id/delete", post(handlers::delete_site))
        .route("/sites/:id/toggle", post(handlers::toggle_site))
        .route("/sites/:id/bind-cert", post(handlers::bind_cert))
        .route("/certs", get(handlers::list_certs))
        .route("/certs/upload", post(handlers::upload_cert))
        .route("/certs/request", post(handlers::request_cert))
        .route("/service", get(handlers::service_page))
        .route("/service/control", post(handlers::service_control))
        .route("/settings", get(handlers::settings_page))
        .route("/settings/password", post(handlers::change_password))
        .route("/logs", get(handlers::logs_page))
        .with_state(state);

    let addr: SocketAddr = cfg.bind_addr.parse()?;
    info!("nginx-manager 已启动，监听地址：http://{}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
