use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
    routing::get,
    Router,
};
use futures_util::{SinkExt, StreamExt};
use openmmo_common::PlayerId;
use openmmo_protocol::{decode_client, encode_server, ClientMessage, ServerMessage};
use tokio::sync::{broadcast, RwLock};
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing::{info, warn};

use crate::content::{default_content_path, load_content_dir};
use crate::economy::assign_ledger_contract;
use crate::quest::quest_journal;
use crate::state::{GameWorld, SharedWorld};
use crate::tick::{handle_client_message, inventory_update, process_tick, snapshot_message};

pub struct AppState {
    pub world: SharedWorld,
    pub broadcast: broadcast::Sender<String>,
}

pub async fn run_server(addr: SocketAddr) -> anyhow::Result<()> {
    let content_path = default_content_path();
    let content = load_content_dir(&content_path).unwrap_or_default();
    let mut world = GameWorld::new(content);
    world.quests.load(&world.content.clone());
    info!(
        "Loaded content: {} items, {} regions",
        world.content.items.len(),
        world.content.regions.len()
    );

    let world = Arc::new(RwLock::new(world));
    let (broadcast, _) = broadcast::channel(1024);

    let state = Arc::new(AppState {
        world: world.clone(),
        broadcast,
    });

    let tick_state = state.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(600));
        loop {
            interval.tick().await;
            let mut messages_to_send = Vec::new();
            {
                let mut w = tick_state.world.write().await;
                let tick_msgs = process_tick(&mut w);
                messages_to_send.extend(tick_msgs);
                let snapshot = snapshot_message(&w, None);
                if let Ok(json) = encode_server(&snapshot) {
                    let _ = tick_state.broadcast.send(json);
                }
            }
            for (_pid, msg) in messages_to_send {
                if let Ok(json) = encode_server(&msg) {
                    let _ = tick_state.broadcast.send(json);
                }
            }
        }
    });

    let app = Router::new()
        .route("/health", get(health))
        .route("/ws", get(ws_handler))
        .route("/api/drops", get(drop_rates))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    info!("OpenMMO server listening on {addr}");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn health() -> &'static str {
    "ok"
}

async fn drop_rates(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let world = state.world.read().await;
    let mut drops = serde_json::Map::new();
    for npc in &world.content.npcs {
        drops.insert(npc.name.clone(), serde_json::json!(npc.loot_table));
    }
    axum::Json(drops)
}

async fn ws_handler(ws: WebSocketUpgrade, State(state): State<Arc<AppState>>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: Arc<AppState>) {
    let (mut sender, mut receiver) = socket.split();
    let mut player_id: Option<PlayerId> = None;
    let mut broadcast_rx = state.broadcast.subscribe();

    let send_task = tokio::spawn(async move {
        loop {
            tokio::select! {
                msg = broadcast_rx.recv() => {
                    match msg {
                        Ok(json) => {
                            if sender.send(Message::Text(json)).await.is_err() {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
            }
        }
    });

    while let Some(Ok(msg)) = receiver.next().await {
        let text = match msg {
            Message::Text(t) => t,
            Message::Close(_) => break,
            _ => continue,
        };

        let client_msg = match decode_client(&text) {
            Ok(m) => m,
            Err(e) => {
                warn!("Bad message: {e}");
                continue;
            }
        };

        match &client_msg {
            ClientMessage::Login {
                username: _,
                character_name,
            } => {
                let mut world = state.world.write().await;
                let pid = world.add_player(character_name.clone());
                let starter_items: Vec<_> = world
                    .content
                    .items
                    .iter()
                    .filter(|item| {
                        item.name.contains("Bronze")
                            || item.name.contains("Log")
                            || item.name.contains("Timber")
                    })
                    .map(|item| (item.id, item.stackable))
                    .collect();
                if let Some(player) = world.players.get_mut(&pid) {
                    for (id, stackable) in starter_items {
                        let _ = player.inventory.add_item(id, 5, stackable);
                    }
                }
                assign_ledger_contract(&mut world, pid);
                let snapshot = snapshot_message(&world, Some(pid));
                let journal = quest_journal(&world, pid);
                drop(world);
                player_id = Some(pid);
                let _ = state.broadcast.send(encode_server(&snapshot).unwrap());
                let login = ServerMessage::LoginResult {
                    success: true,
                    player_id: Some(pid),
                    message: "Welcome to OpenMMO".into(),
                };
                let inv = {
                    let world = state.world.read().await;
                    world.players.get(&pid).map(inventory_update)
                };
                if let Some(inv) = inv {
                    let _ = state.broadcast.send(encode_server(&inv).unwrap());
                }
                let _ = state.broadcast.send(encode_server(&login).unwrap());
                let _ = journal;
            }
            _ => {
                if let Some(pid) = player_id {
                    let mut world = state.world.write().await;
                    let responses = handle_client_message(&mut world, pid, client_msg);
                    drop(world);
                    for resp in responses {
                        if let Ok(json) = encode_server(&resp) {
                            let _ = state.broadcast.send(json);
                        }
                    }
                }
            }
        }
    }

    if let Some(pid) = player_id {
        let mut world = state.world.write().await;
        world.remove_player(pid);
    }
    send_task.abort();
}
