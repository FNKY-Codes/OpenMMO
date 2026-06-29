use std::collections::HashMap;
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
use tokio::sync::{mpsc, RwLock};
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing::{info, warn};

use crate::content::{default_content_path, load_content_dir};
use crate::economy::assign_ledger_contract;
use crate::persistence;
use crate::quest::quest_journal;
use crate::state::{GameWorld, SharedWorld};
use crate::tick::{handle_client_message, inventory_update, process_tick, snapshot_message, MessageTarget};

pub struct AppState {
    pub world: SharedWorld,
    pub player_senders: Arc<RwLock<HashMap<PlayerId, mpsc::UnboundedSender<String>>>>,
    pub broadcast_senders: Arc<RwLock<HashMap<u64, mpsc::UnboundedSender<String>>>>,
    pub next_conn_id: Arc<RwLock<u64>>,
}

impl AppState {
    pub async fn send_to_player(&self, player_id: PlayerId, msg: &ServerMessage) {
        if let Ok(json) = encode_server(msg) {
            if let Some(tx) = self.player_senders.read().await.get(&player_id) {
                let _ = tx.send(json);
            }
        }
    }

    pub async fn broadcast(&self, msg: &ServerMessage) {
        if let Ok(json) = encode_server(msg) {
            for tx in self.broadcast_senders.read().await.values() {
                let _ = tx.send(json.clone());
            }
        }
    }

    /// Push a state update to every logged-in player in `region_id` except `skip`.
    pub async fn notify_region(
        &self,
        region_id: openmmo_common::RegionId,
        skip: PlayerId,
        msg: &ServerMessage,
    ) {
        if let Ok(json) = encode_server(msg) {
            let world = self.world.read().await;
            let senders = self.player_senders.read().await;
            for (pid, tx) in senders.iter() {
                if *pid == skip {
                    continue;
                }
                if world
                    .players
                    .get(pid)
                    .is_some_and(|p| p.region_id == region_id)
                {
                    let _ = tx.send(json.clone());
                }
            }
        }
    }

    pub async fn register_connection(&self, tx: mpsc::UnboundedSender<String>) -> u64 {
        let mut next = self.next_conn_id.write().await;
        *next += 1;
        let id = *next;
        self.broadcast_senders.write().await.insert(id, tx);
        id
    }

    pub async fn unregister_connection(&self, conn_id: u64) {
        self.broadcast_senders.write().await.remove(&conn_id);
    }

    pub async fn register_player(
        &self,
        player_id: PlayerId,
        tx: mpsc::UnboundedSender<String>,
    ) {
        self.player_senders.write().await.insert(player_id, tx);
    }

    pub async fn unregister_player(&self, player_id: PlayerId) {
        self.player_senders.write().await.remove(&player_id);
    }
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
    crate::content::spawn_hot_reload_task(world.clone());
    let state = Arc::new(AppState {
        world: world.clone(),
        player_senders: Arc::new(RwLock::new(HashMap::new())),
        broadcast_senders: Arc::new(RwLock::new(HashMap::new())),
        next_conn_id: Arc::new(RwLock::new(0)),
    });

    let tick_state = state.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(600));
        let mut full_snapshot_counter = 0u64;
        loop {
            interval.tick().await;
            let mut tick_messages = Vec::new();
            {
                let mut w = tick_state.world.write().await;
                tick_messages = process_tick(&mut w);
                full_snapshot_counter += 1;
                let delta = ServerMessage::StateDelta {
                    tick: w.tick,
                    entities: w.entities_snapshot(),
                };
                tick_state.broadcast(&delta).await;
                if full_snapshot_counter % 50 == 0 {
                    let player_ids: Vec<PlayerId> = tick_state
                        .player_senders
                        .read()
                        .await
                        .keys()
                        .copied()
                        .collect();
                    for pid in player_ids {
                        let snapshot = snapshot_message(&w, Some(pid));
                        tick_state.send_to_player(pid, &snapshot).await;
                    }
                }
            }
            for (target, msg) in tick_messages {
                match target {
                    MessageTarget::Player(pid) => {
                        tick_state.send_to_player(pid, &msg).await;
                    }
                    MessageTarget::Broadcast => {
                        tick_state.broadcast(&msg).await;
                    }
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
    let (conn_tx, mut conn_rx) = mpsc::unbounded_channel::<String>();
    let conn_id = state.register_connection(conn_tx.clone()).await;

    let send_task = tokio::spawn(async move {
        loop {
            tokio::select! {
                msg = conn_rx.recv() => {
                    match msg {
                        Some(json) => {
                            if sender.send(Message::Text(json)).await.is_err() {
                                break;
                            }
                        }
                        None => break,
                    }
                }
            }
        }
    });

    let mut player_id: Option<PlayerId> = None;

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
                username,
                character_name,
                password,
            } => {
                let mut world = state.world.write().await;
                if !crate::anticheat::validate_username(username) {
                    drop(world);
                    let login = ServerMessage::LoginResult {
                        success: false,
                        player_id: None,
                        message: "Invalid username".into(),
                    };
                    if let Ok(json) = encode_server(&login) {
                        let _ = conn_tx.send(json);
                    }
                    continue;
                }
                if let Ok(db_url) = std::env::var("DATABASE_URL") {
                    let authed = persistence::authenticate_account(&db_url, username, password)
                        .await
                        .unwrap_or(false);
                    if !authed {
                        drop(world);
                        let login = ServerMessage::LoginResult {
                            success: false,
                            player_id: None,
                            message: "Invalid credentials".into(),
                        };
                        if let Ok(json) = encode_server(&login) {
                            let _ = conn_tx.send(json);
                        }
                        continue;
                    }
                }
                let pid = if let Ok(db_url) = std::env::var("DATABASE_URL") {
                    persistence::load_or_create_player(&db_url, username, character_name, &mut world)
                        .await
                        .unwrap_or_else(|_| world.add_player(character_name.clone()))
                } else {
                    world.add_player(character_name.clone())
                };
                world.nudge_player_if_overlapping(pid);
                if let Ok(redis_url) = std::env::var("REDIS_URL") {
                    let _ = crate::session::store_session(&redis_url, username, pid).await;
                }
                if world.players.get(&pid).is_some_and(|p| p.inventory.slots.iter().all(|s| s.is_none()))
                {
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
                }
                assign_ledger_contract(&mut world, pid);
                let region_id = world.players.get(&pid).map(|p| p.region_id);
                let tick = world.tick;
                let snapshot = snapshot_message(&world, Some(pid));
                let journal = quest_journal(&world, pid);
                let inv = world.players.get(&pid).map(inventory_update);
                drop(world);

                player_id = Some(pid);
                state.register_player(pid, conn_tx.clone()).await;

                if let Ok(json) = encode_server(&snapshot) {
                    let _ = conn_tx.send(json);
                }
                if let Some(region_id) = region_id {
                    let join_delta = ServerMessage::StateDelta {
                        tick,
                        entities: {
                            let world = state.world.read().await;
                            world.entities_snapshot()
                        },
                    };
                    state.notify_region(region_id, pid, &join_delta).await;
                }
                if let Some(inv) = inv {
                    if let Ok(json) = encode_server(&inv) {
                        let _ = conn_tx.send(json);
                    }
                }
                let login = ServerMessage::LoginResult {
                    success: true,
                    player_id: Some(pid),
                    message: "Welcome to OpenMMO".into(),
                };
                if let Ok(json) = encode_server(&login) {
                    let _ = conn_tx.send(json);
                }
                let journal_msg = ServerMessage::QuestJournal { entries: journal };
                if let Ok(json) = encode_server(&journal_msg) {
                    let _ = conn_tx.send(json);
                }
            }
            _ => {
                if let Some(pid) = player_id {
                    let mut world = state.world.write().await;
                    let responses = handle_client_message(&mut world, pid, client_msg);
                    drop(world);
                    for (target, resp) in responses {
                        match target {
                            MessageTarget::Player(pid) => {
                                state.send_to_player(pid, &resp).await;
                            }
                            MessageTarget::Broadcast => {
                                state.broadcast(&resp).await;
                            }
                        }
                    }
                }
            }
        }
    }

    if let Some(pid) = player_id {
        if let Ok(db_url) = std::env::var("DATABASE_URL") {
            let world = state.world.read().await;
            if let Some(player) = world.players.get(&pid) {
                let _ = persistence::save_player(&db_url, player).await;
            }
        }
        state.unregister_player(pid).await;
        let mut world = state.world.write().await;
        world.remove_player(pid);
    }
    state.unregister_connection(conn_id).await;
    send_task.abort();
}
