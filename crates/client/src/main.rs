mod net;

use std::sync::mpsc;
use std::thread;

use openmmo_common::load_content;
use openmmo_engine::{EngineApp, NetCommand};
use openmmo_protocol::ServerMessage;
use tracing_subscriber::EnvFilter;

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let content_path = std::path::PathBuf::from(
        std::env::var("OPENMMO_CONTENT").unwrap_or_else(|_| "content".to_string()),
    );
    let content = load_content(&content_path).unwrap_or_default();
    let region = content.regions.first().cloned();

    let (net_tx, net_rx) = mpsc::channel::<NetCommand>();
    let (msg_tx, msg_rx) = mpsc::channel::<ServerMessage>();

    thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(net::network_thread(net_rx, msg_tx));
    });

    let mut app = EngineApp::default();
    app.content = content;
    app.region = region;
    app.net_tx = Some(net_tx);
    app.net_rx = Some(msg_rx);

    app.run()
}
