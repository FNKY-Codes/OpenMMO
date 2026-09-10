mod net;

use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;

use openmmo_common::{load_content, RegionId};
use openmmo_engine::{EngineApp, NetCommand};
use openmmo_protocol::ServerMessage;
use tracing_subscriber::EnvFilter;

/// Server URL baked in at build time by the release workflow. Falls back to a
/// local dev server when unset so `cargo run` keeps working.
const DEFAULT_SERVER_URL: &str = match option_env!("OPENMMO_SERVER_URL") {
    Some(url) => url,
    None => "ws://127.0.0.1:8080/ws",
};

/// Locate a bundled directory (`content/`, `assets/`) for a shipped build.
///
/// Order: explicit env var, then next to the executable, then the current
/// working directory (the `cargo run` case).
fn bundled_dir(env_var: &str, name: &str) -> PathBuf {
    if let Ok(p) = std::env::var(env_var) {
        return PathBuf::from(p);
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let beside_exe = dir.join(name);
            if beside_exe.is_dir() {
                return beside_exe;
            }
        }
    }
    PathBuf::from(name)
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let content_path = bundled_dir("OPENMMO_CONTENT", "content");
    let assets_path = bundled_dir("OPENMMO_ASSETS", "assets");
    // The engine reads OPENMMO_ASSETS itself; make the resolved path visible to it.
    if std::env::var_os("OPENMMO_ASSETS").is_none() {
        std::env::set_var("OPENMMO_ASSETS", &assets_path);
    }

    let content = match load_content(&content_path) {
        Ok(c) => c,
        Err(e) => {
            tracing::error!(
                "Failed to load game content from {}: {e:#}. The client will not render the world.",
                content_path.display()
            );
            Default::default()
        }
    };
    let region = content.regions.first().cloned();

    let (net_tx, net_rx) = mpsc::channel::<NetCommand>();
    let (msg_tx, msg_rx) = mpsc::channel::<ServerMessage>();

    thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(net::network_thread(net_rx, msg_tx));
    });

    let mut app = EngineApp::default();
    app.content = content;
    app.region = region.clone();
    app.current_region_id = region.as_ref().map(|r| r.id).unwrap_or(RegionId(1));
    app.net_tx = Some(net_tx);
    app.net_rx = Some(msg_rx);
    app.ui.connection_url =
        std::env::var("OPENMMO_SERVER_URL").unwrap_or_else(|_| DEFAULT_SERVER_URL.to_string());

    app.run()
}
