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
use tokio::sync::{mpsc, oneshot, RwLock};
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing::{info, warn};

use crate::content::{default_content_path, load_content_dir};
use crate::economy::assign_ledger_contract;
use crate::persistence::{self, AuthResult, CharacterError};
use crate::quest::quest_journal;
use crate::state::{GameWorld, SharedWorld};
use crate::tick::{
    handle_client_message, inventory_update, process_tick, snapshot_message, MessageTarget,
};

/// How often every online player is flushed to the database.
const AUTOSAVE_INTERVAL: Duration = Duration::from_secs(60);

pub struct AppState {
    pub world: SharedWorld,
    pub player_senders: Arc<RwLock<HashMap<PlayerId, mpsc::UnboundedSender<String>>>>,
    /// Fired to force-disconnect a player's socket (used when the same
    /// character logs in from a second connection).
    pub player_kicks: Arc<RwLock<HashMap<PlayerId, oneshot::Sender<String>>>>,
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
        kick: oneshot::Sender<String>,
    ) {
        self.player_senders.write().await.insert(player_id, tx);
        self.player_kicks.write().await.insert(player_id, kick);
    }

    pub async fn unregister_player(&self, player_id: PlayerId) {
        self.player_senders.write().await.remove(&player_id);
        self.player_kicks.write().await.remove(&player_id);
    }

    /// If `character_name` is already online, save it, tell that socket to
    /// close, and remove the player from the world so the new login can take
    /// over. Returns true when a session was evicted.
    async fn evict_existing_session(&self, character_name: &str) -> bool {
        let existing = {
            let world = self.world.read().await;
            world
                .players
                .values()
                .find(|p| p.name.eq_ignore_ascii_case(character_name))
                .map(|p| p.id)
        };
        let Some(old_pid) = existing else {
            return false;
        };
        if let Some(kick) = self.player_kicks.write().await.remove(&old_pid) {
            let _ = kick.send("Logged in from another location".to_string());
        }
        let snapshot = {
            let world = self.world.read().await;
            world.players.get(&old_pid).cloned()
        };
        if let Some(player) = snapshot {
            if let Err(e) = persistence::save_player(&player).await {
                warn!("failed to save evicted session {}: {e}", player.name);
            }
        }
        self.unregister_player(old_pid).await;
        self.world.write().await.remove_player(old_pid);
        true
    }

    /// Persist every online player. Returns how many rows were written.
    pub async fn save_all_players(&self) -> usize {
        if !persistence::enabled() {
            return 0;
        }
        let players: Vec<_> = {
            let world = self.world.read().await;
            world.players.values().cloned().collect()
        };
        persistence::save_all(&players).await
    }
}

pub async fn run_server(addr: SocketAddr) -> anyhow::Result<()> {
    let content_path = default_content_path();
    let content = match load_content_dir(&content_path) {
        Ok(c) => c,
        Err(e) => {
            warn!(
                "Failed to load content from {}: {e:#}. Starting with an empty world.",
                content_path.display()
            );
            Default::default()
        }
    };
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
        player_kicks: Arc::new(RwLock::new(HashMap::new())),
        broadcast_senders: Arc::new(RwLock::new(HashMap::new())),
        next_conn_id: Arc::new(RwLock::new(0)),
    });

    let tick_state = state.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(600));
        let mut full_snapshot_counter = 0u64;
        loop {
            interval.tick().await;
            let tick_messages;
            {
                let mut w = tick_state.world.write().await;
                tick_messages = process_tick(&mut w);
                full_snapshot_counter += 1;
                let delta = ServerMessage::StateDelta {
                    tick: w.tick,
                    entities: w.entities_snapshot(),
                };
                tick_state.broadcast(&delta).await;
                if full_snapshot_counter.is_multiple_of(50) {
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

    if persistence::enabled() {
        let save_state = state.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(AUTOSAVE_INTERVAL);
            interval.tick().await; // first tick fires immediately; skip it
            loop {
                interval.tick().await;
                let n = save_state.save_all_players().await;
                if n > 0 {
                    info!("Autosaved {n} player(s)");
                }
            }
        });
    }

    let app = Router::new()
        .route("/health", get(health))
        .route("/ws", get(ws_handler))
        .route("/api/drops", get(drop_rates))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state.clone());

    info!("OpenMMO server listening on {addr}");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    // Flush everyone once the listener has stopped accepting connections.
    let n = state.save_all_players().await;
    info!("Shutdown: saved {n} player(s)");
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let terminate = async {
        use tokio::signal::unix::{signal, SignalKind};
        match signal(SignalKind::terminate()) {
            Ok(mut s) => {
                s.recv().await;
            }
            Err(_) => std::future::pending::<()>().await,
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    info!("Shutdown signal received");
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

fn login_failure(message: &str) -> ServerMessage {
    ServerMessage::LoginResult {
        success: false,
        player_id: None,
        message: message.to_string(),
        content_fingerprint: String::new(),
    }
}

fn send_json(tx: &mpsc::UnboundedSender<String>, msg: &ServerMessage) {
    if let Ok(json) = encode_server(msg) {
        let _ = tx.send(json);
    }
}

async fn handle_socket(socket: WebSocket, state: Arc<AppState>) {
    let (mut sender, mut receiver) = socket.split();
    let (conn_tx, mut conn_rx) = mpsc::unbounded_channel::<String>();
    let conn_id = state.register_connection(conn_tx.clone()).await;

    let send_task = tokio::spawn(async move {
        while let Some(json) = conn_rx.recv().await {
            if sender.send(Message::Text(json)).await.is_err() {
                break;
            }
        }
        let _ = sender.close().await;
    });

    let mut player_id: Option<PlayerId> = None;
    // Resolved when another login evicts this session. Replaced on each login.
    let (_noop_tx, mut kick_rx) = oneshot::channel::<String>();

    loop {
        let msg = tokio::select! {
            biased;
            reason = &mut kick_rx => {
                if let Ok(reason) = reason {
                    send_json(&conn_tx, &ServerMessage::Error { message: reason });
                }
                // The evictor already saved and removed the player.
                player_id = None;
                break;
            }
            msg = receiver.next() => msg,
        };
        let Some(Ok(msg)) = msg else { break };

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
                if player_id.is_some() {
                    send_json(&conn_tx, &login_failure("Already logged in"));
                    continue;
                }
                if !crate::anticheat::validate_username(username) {
                    send_json(&conn_tx, &login_failure("Invalid username"));
                    continue;
                }
                if !crate::anticheat::validate_username(character_name) {
                    send_json(&conn_tx, &login_failure("Invalid character name"));
                    continue;
                }
                if persistence::enabled() && password.is_empty() {
                    send_json(&conn_tx, &login_failure("Password required"));
                    continue;
                }

                match persistence::authenticate_account(username, password).await {
                    Ok(AuthResult::Ok) => {}
                    Ok(AuthResult::Created) => info!("Created account {username}"),
                    Ok(AuthResult::BadPassword) => {
                        send_json(&conn_tx, &login_failure("Invalid credentials"));
                        continue;
                    }
                    Err(e) => {
                        warn!("auth error for {username}: {e}");
                        send_json(&conn_tx, &login_failure("Login temporarily unavailable"));
                        continue;
                    }
                }

                if state.evict_existing_session(character_name).await {
                    info!("Evicted previous session for {character_name}");
                }

                let mut world = state.world.write().await;
                let pid =
                    match persistence::load_or_create_player(username, character_name, &mut world)
                        .await
                    {
                        Ok(Ok(pid)) => pid,
                        Ok(Err(CharacterError::OwnedByOther)) => {
                            drop(world);
                            send_json(
                                &conn_tx,
                                &login_failure("That character name belongs to another account"),
                            );
                            continue;
                        }
                        Err(e) => {
                            drop(world);
                            warn!("load_or_create_player failed for {character_name}: {e}");
                            send_json(&conn_tx, &login_failure("Login temporarily unavailable"));
                            continue;
                        }
                    };
                world.nudge_player_if_overlapping(pid);
                if let Ok(redis_url) = std::env::var("REDIS_URL") {
                    let _ = crate::session::store_session(&redis_url, username, pid).await;
                }
                if world
                    .players
                    .get(&pid)
                    .is_some_and(|p| p.inventory.slots.iter().all(|s| s.is_none()))
                {
                    // Fresh character: hand out the content pack's starter kit.
                    let starter_items: Vec<_> = world
                        .content
                        .starter_kit
                        .iter()
                        .filter_map(|entry| {
                            world
                                .content
                                .item(entry.item_id)
                                .map(|item| (item.id, entry.quantity, item.stackable))
                        })
                        .collect();
                    if let Some(player) = world.players.get_mut(&pid) {
                        for (id, quantity, stackable) in starter_items {
                            let _ = player.inventory.add_item(id, quantity, stackable);
                        }
                    }
                }
                assign_ledger_contract(&mut world, pid);
                let region_id = world.players.get(&pid).map(|p| p.region_id);
                let tick = world.tick;
                let snapshot = snapshot_message(&world, Some(pid));
                let journal = quest_journal(&world, pid);
                let inv = world.players.get(&pid).map(inventory_update);
                let content_fingerprint = world.content.fingerprint();
                drop(world);

                player_id = Some(pid);
                let (kick_tx, new_kick_rx) = oneshot::channel();
                kick_rx = new_kick_rx;
                state.register_player(pid, conn_tx.clone(), kick_tx).await;

                send_json(&conn_tx, &snapshot);
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
                    send_json(&conn_tx, &inv);
                }
                send_json(
                    &conn_tx,
                    &ServerMessage::LoginResult {
                        success: true,
                        player_id: Some(pid),
                        message: "Welcome to OpenMMO".into(),
                        content_fingerprint,
                    },
                );
                send_json(&conn_tx, &ServerMessage::QuestJournal { entries: journal });
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
        let snapshot = {
            let world = state.world.read().await;
            world.players.get(&pid).cloned()
        };
        if let Some(player) = snapshot {
            if let Err(e) = persistence::save_player(&player).await {
                warn!("failed to save {} on disconnect: {e}", player.name);
            }
        }
        state.unregister_player(pid).await;
        let mut world = state.world.write().await;
        world.remove_player(pid);
    }
    state.unregister_connection(conn_id).await;
    // Every clone of `conn_tx` has now been dropped from the registries, so
    // dropping ours closes the channel; the send task drains anything still
    // queued (e.g. the eviction notice) and sends a close frame.
    drop(conn_tx);
    let _ = tokio::time::timeout(Duration::from_secs(5), send_task).await;
}
