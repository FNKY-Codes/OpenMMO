mod anticheat;
mod combat;
mod content;
mod economy;
mod minigame;
mod pathfinding;
mod persistence;
mod quest;
mod session;
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

    // `PORT` is what most PaaS hosts (Railway, Fly, Heroku) inject; `OPENMMO_BIND`
    // wins when set so self-hosters can pin an interface.
    let bind = std::env::var("OPENMMO_BIND").unwrap_or_else(|_| {
        let port = std::env::var("PORT").unwrap_or_else(|_| "8080".to_string());
        format!("0.0.0.0:{port}")
    });
    let addr: SocketAddr = bind.parse()?;

    match std::env::var("DATABASE_URL") {
        Ok(db_url) => {
            persistence::init(&db_url).await?;
            if persistence::enabled() {
                tracing::info!("PostgreSQL connected; migrations applied");
            }
        }
        Err(_) => tracing::warn!(
            "DATABASE_URL not set: running in memory-only mode, progress will not persist"
        ),
    }

    ws::run_server(addr).await
}
