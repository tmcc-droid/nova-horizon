//! Nova Horizon game-server — full MVP modular monolith.

mod auth_api;
mod config;
mod gameplay;
mod jwt_util;
mod password;
mod rate_limit;
mod sim_hub;
mod state;
mod tokens;
mod ws;

use std::env;

use axum::routing::{get, post};
use axum::Router;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing::info;
use tracing_subscriber::EnvFilter;

use crate::config::Config;
use crate::sim_hub::start_sim;
use crate::state::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();

    let args: Vec<String> = env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("migrate") {
        return run_migrate().await;
    }

    let config = Config::from_env()?;
    info!(
        bind = %config.bind,
        content_version = %config.content_version,
        protocol_version = protocol::PROTOCOL_VERSION,
        "starting nova-horizon MVP server"
    );

    let pool = db::connect_and_migrate(&config.database_url).await?;
    info!(
        market_rows = db::count_market_rows(&pool).await.unwrap_or(0),
        "database ready"
    );

    let content = content::load_default()?;
    let sim = start_sim(content);
    let state = AppState::new(pool, config.clone(), sim);

    let app = Router::new()
        .route("/health", get(auth_api::health))
        .route("/metrics", get(metrics))
        .route("/content/manifest", get(auth_api::content_manifest))
        .route("/galaxy", get(auth_api::galaxy_chart))
        .route("/auth/register", post(auth_api::register))
        .route("/auth/login", post(auth_api::login))
        .route("/auth/refresh", post(auth_api::refresh))
        .route("/auth/play", post(auth_api::play))
        .route("/characters", get(auth_api::list_characters))
        .route("/characters", post(auth_api::create_character))
        .route("/ws", get(ws::ws_upgrade))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(config.bind).await?;
    info!(%config.bind, "listening HTTP+WS (dock/trade/combat/jump/NPC)");
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(async {
        let _ = tokio::signal::ctrl_c().await;
        info!("shutdown");
    })
    .await?;
    Ok(())
}

async fn metrics() -> String {
    // Minimal Prometheus text for PR-16.
    format!(
        "# HELP nova_up 1 if process is up\n# TYPE nova_up gauge\nnova_up 1\n# TYPE nova_sim_tick_hz gauge\nnova_sim_tick_hz {}\n",
        sim::TICK_HZ
    )
}

async fn run_migrate() -> anyhow::Result<()> {
    let url = env::var("DATABASE_URL")?;
    let pool = db::connect_and_migrate(&url).await?;
    info!(
        market_rows = db::count_market_rows(&pool).await?,
        "migrate OK"
    );
    Ok(())
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,game_server=info,db=info"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .compact()
        .init();
}
