mod anticheat;
mod combat;
mod content;
mod economy;
mod minigame;
mod pathfinding;
mod persistence;
mod quest;
mod social;
mod state;
mod tick;
mod ws;

use std::net::SocketAddr;

use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::from_default_env().add_directive("openmmo_server=info".parse()?),
        )
        .init();

    let addr: SocketAddr = std::env::var("OPENMMO_BIND")
        .unwrap_or_else(|_| "0.0.0.0:8080".to_string())
        .parse()?;

    if let Ok(db_url) = std::env::var("DATABASE_URL") {
        persistence::run_migrations(&db_url).await?;
        tracing::info!("PostgreSQL migrations applied");
    }

    ws::run_server(addr).await
}
