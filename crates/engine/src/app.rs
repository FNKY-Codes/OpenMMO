use std::collections::HashMap;
use std::sync::mpsc::{Receiver, Sender};
use std::time::{Duration, Instant};

use egui_wgpu::wgpu;
use egui_wgpu::Renderer as EguiRenderer;
use egui_winit::winit;
use egui_winit::State as EguiWinitState;
use openmmo_common::{ContentPack, EntityKind, Equipment, NpcFootprint, RegionDef, RegionId, WorldEntity};
use openmmo_protocol::{ClientMessage, ServerMessage};

use egui_winit::egui;

use crate::{
    animation::{AnimationPlayer, DeathCorpse, FROG_ATTACK, FROG_DEATH, FROG_IDLE},
    entity_bounds,
    input::InputState,
    mesh::{default_assets_dir, ModelCache},
    model::resolve_player_model_path,
    movement_interp::EntityMovementInterp,
    renderer::{HoverTarget, Renderer},
    ui::{
        build_context_menu, draw_combat_health_bars, setup_theme, to_client_message, GameUi,
        UiAction,
    },
};

#[derive(Debug)]
pub enum NetCommand {
    Connect {
        url: String,
        username: String,
        character: String,
        password: String,
    },
    Send(ClientMessage),
}

pub struct EngineApp {
    input: InputState,
    pub ui: GameUi,
    pub content: ContentPack,
    pub model_cache: ModelCache,
    pub entities: Vec<WorldEntity>,
    pub movement_interp: EntityMovementInterp,
    pub region: Option<RegionDef>,
    pub current_region_id: RegionId,
    pub local_player: Option<openmmo_common::PlayerId>,
    pub inventory: openmmo_common::Inventory,
    pub bank: openmmo_common::Inventory,
    pub equipment: Equipment,
    pub skills: openmmo_common::SkillBook,
    pub hp: u32,
    pub max_hp: u32,
    pub quest_text: Vec<(String, String)>,
    pub combat_opponent: Option<openmmo_common::EntityId>,
    pub net_tx: Option<Sender<NetCommand>>,
    pub net_rx: Option<Receiver<ServerMessage>>,
    pointer_over_ui: bool,
    pub player_model_path: Option<std::path::PathBuf>,
    npc_animations: HashMap<openmmo_common::EntityId, AnimationPlayer>,
    death_corpses: Vec<DeathCorpse>,
    last_frame: Instant,
}

impl Default for EngineApp {
    fn default() -> Self {
        Self {
            input: InputState::default(),
            ui: GameUi::default(),
            content: ContentPack::default(),
            model_cache: ModelCache::new(default_assets_dir()),
            entities: Vec::new(),
            movement_interp: EntityMovementInterp::default(),
            region: None,
            current_region_id: RegionId(1),
            local_player: None,
            inventory: openmmo_common::Inventory::new(openmmo_common::INVENTORY_SIZE),
            bank: openmmo_common::Inventory::new(openmmo_common::BANK_SIZE),
            equipment: Equipment::default(),
            skills: openmmo_common::SkillBook::new_mvp(),
            hp: 10,
            max_hp: 10,
            quest_text: Vec::new(),
            combat_opponent: None,
            net_tx: None,
            net_rx: None,
            pointer_over_ui: false,
            player_model_path: resolve_player_model_path(),
            npc_animations: HashMap::new(),
            death_corpses: Vec::new(),
            last_frame: Instant::now(),
        }
    }
}

impl EngineApp {
    pub fn run(mut self) -> anyhow::Result<()> {
        let event_loop = winit::event_loop::EventLoop::new()?;
        let window = std::sync::Arc::new(
            event_loop
                .create_window(
                    winit::window::Window::default_attributes()
                        .with_title("OpenMMO")
                        .with_inner_size(winit::dpi::LogicalSize::new(1280, 720)),
                )
                .map_err(|e| anyhow::anyhow!("{e}"))?,
        );
        let mut renderer = pollster::block_on(Renderer::new(
            window.clone(),
            self.player_model_path.as_deref(),
            &self.content,
        ));
        let egui_ctx = egui::Context::default();
        setup_theme(&egui_ctx);
        let mut egui_state = EguiWinitState::new(
            egui_ctx.clone(),
            egui::ViewportId::ROOT,
            &window,
            Some(window.scale_factor() as f32),
            None,
            None,
        );
        let mut egui_renderer = EguiRenderer::new(
            &renderer.gpu().device,
            renderer.gpu().config.format,
            None,
            1,
            false,
        );

        event_loop.run(move |event, elwt| {
            elwt.set_control_flow(winit::event_loop::ControlFlow::Poll);

            match event {
                winit::event::Event::WindowEvent { event, .. } => {
                    let response = egui_state.on_window_event(&window, &event);
                    match event {
                        winit::event::WindowEvent::CloseRequested => elwt.exit(),
                        winit::event::WindowEvent::Resized(size) => {
                            renderer.resize(size.width, size.height)
                        }
                        winit::event::WindowEvent::CursorMoved { position, .. } => {
                            self.input.mouse_x = position.x as f32;
                            self.input.mouse_y = position.y as f32;

                            if !response.consumed && self.input.middle_dragging {
                                let (dx, dy) = self.input.drag_delta();
                                renderer.camera_mut().rotate(-dx * 0.005, dy * 0.005);
                            }
                        }
                        winit::event::WindowEvent::MouseInput { state, button, .. } => {
                            if !response.consumed {
                                match (state, button) {
                                    (
                                        winit::event::ElementState::Pressed,
                                        winit::event::MouseButton::Left,
                                    ) => {
                                        self.input.left_clicked = true;
                                    }
                                    (
                                        winit::event::ElementState::Pressed,
                                        winit::event::MouseButton::Right,
                                    ) => {
                                        self.input.right_clicked = true;
                                    }
                                    (
                                        winit::event::ElementState::Pressed,
                                        winit::event::MouseButton::Middle,
                                    ) => {
                                        self.input.middle_dragging = true;
                                        self.input.begin_drag();
                                    }
                                    (
                                        winit::event::ElementState::Released,
                                        winit::event::MouseButton::Middle,
                                    ) => {
                                        self.input.middle_dragging = false;
                                    }
                                    _ => {}
                                }
                            }
                        }
                        winit::event::WindowEvent::MouseWheel { delta, .. } => {
                            if !response.consumed {
                                let scroll = match delta {
                                    winit::event::MouseScrollDelta::LineDelta(_, y) => y * 3.0,
                                    winit::event::MouseScrollDelta::PixelDelta(pos) => {
                                        pos.y as f32 * 0.15
                                    }
                                };
                                self.input.scroll_delta += scroll;
                            }
                        }
                        winit::event::WindowEvent::RedrawRequested => {
                            self.render_frame(
                                &egui_ctx,
                                &mut egui_state,
                                &mut egui_renderer,
                                &mut renderer,
                                &window,
                            );
                        }
                        _ => {}
                    }
                }
                winit::event::Event::AboutToWait => {
                    window.request_redraw();
                }
                _ => {}
            }
        })?;

        Ok(())
    }

    pub fn poll_network(&mut self) {
        let messages: Vec<ServerMessage> = if let Some(rx) = &self.net_rx {
            rx.try_iter().collect()
        } else {
            Vec::new()
        };
        for msg in messages {
            self.apply_server_message(msg);
        }
    }

    fn send(&mut self, msg: ClientMessage) {
        match &msg {
            ClientMessage::Attack { target, .. } | ClientMessage::CastSpell { target, .. } => {
                self.combat_opponent = Some(*target);
            }
            ClientMessage::WalkIntent { .. } => {
                self.combat_opponent = None;
            }
            _ => {}
        }
        if let Some(tx) = &self.net_tx {
            let _ = tx.send(NetCommand::Send(msg));
        }
    }

    fn apply_server_message(&mut self, msg: ServerMessage) {
        match msg {
            ServerMessage::LoginResult {
                success,
                player_id,
                message,
            } => {
                self.ui.connected = success;
                self.ui.status = message;
                if success {
                    self.local_player = player_id;
                }
            }
            ServerMessage::WorldSnapshot {
                entities,
                local_player,
                region_id,
                tick,
                ..
            } => {
                self.current_region_id = region_id;
                self.region = self.content.region(region_id).cloned();
                self.entities = entities;
                if let Some(lp) = local_player {
                    self.local_player = Some(lp);
                }
                let now = Instant::now();
                let _ = tick;
                self.seed_movement_interp(now);
                self.sync_local_hp();
            }
            ServerMessage::RegionChanged {
                region_id,
                position,
                entities,
            } => {
                self.current_region_id = region_id;
                self.region = self.content.region(region_id).cloned();
                self.entities = entities;
                if let Some(lp) = self.local_player {
                    for entity in &mut self.entities {
                        if let EntityKind::Player {
                            player_id,
                            position: pos,
                            ..
                        } = &mut entity.kind
                        {
                            if *player_id == lp {
                                *pos = position;
                            }
                        }
                    }
                }
                let now = Instant::now();
                self.seed_movement_interp(now);
                self.sync_local_hp();
            }
            ServerMessage::StateDelta { entities, .. } => {
                let now = Instant::now();
                self.merge_entities(entities, now);
                self.sync_local_hp();
            }
            ServerMessage::PlayerUpdate {
                player_id,
                position,
                hp,
                max_hp,
                ..
            } => {
                let now = Instant::now();
                for entity in &mut self.entities {
                    if let EntityKind::Player {
                        player_id: pid,
                        position: pos,
                        hp: ehp,
                        max_hp: emhp,
                        ..
                    } = &mut entity.kind
                    {
                        if *pid == player_id {
                            self.movement_interp.on_position_change(
                                entity.entity_id,
                                position,
                                now,
                                self.region.as_ref(),
                                None,
                            );
                            *pos = position;
                            *ehp = hp;
                            *emhp = max_hp;
                        }
                    }
                }
                if self.local_player == Some(player_id) {
                    self.hp = hp;
                    self.max_hp = max_hp;
                }
            }
            ServerMessage::InventoryUpdate {
                inventory,
                bank,
                equipment,
            } => {
                self.inventory = inventory;
                self.bank = bank;
                self.equipment = equipment;
            }
            ServerMessage::SkillUpdate {
                skills,
                levels_gained,
            } => {
                self.skills = skills;
                for (skill, level) in levels_gained {
                    self.ui.status = format!("{} level up: {level}", skill.name());
                    let _ = (skill, level);
                }
            }
            ServerMessage::ChatMessage { from, message, .. } => {
                self.ui.chat_log.push((from, message));
            }
            ServerMessage::XpDrop { skill, amount } => {
                self.ui.xp_drops.push((
                    skill.name().to_string(),
                    amount,
                    std::time::Instant::now(),
                ));
            }
            ServerMessage::Damage {
                source,
                target,
                amount,
                ..
            } => {
                self.ui.combat_log.push(format!(
                    "Damage: {:?} -> {:?} ({amount})",
                    source.0, target.0
                ));
                if let Some(local_eid) = self.local_entity_id() {
                    if source == local_eid {
                        self.combat_opponent = Some(target);
                    } else if target == local_eid {
                        self.combat_opponent = Some(source);
                    }
                }
                if let Some(entity) = self.entities.iter().find(|e| e.entity_id == source) {
                    if let EntityKind::Npc { npc_id, .. } = &entity.kind {
                        if self
                            .content
                            .npc(*npc_id)
                            .and_then(|d| d.model.as_deref())
                            .is_some()
                        {
                            self.npc_animations
                                .entry(source)
                                .or_insert_with(|| AnimationPlayer::new(FROG_IDLE))
                                .play(FROG_ATTACK, false);
                        }
                    }
                }
                self.sync_local_hp();
            }
            ServerMessage::Death { entity, .. } => {
                self.ui.combat_log.push(format!("Entity {} died", entity.0));
                if let Some(corpse) = self.capture_death_corpse(entity) {
                    self.death_corpses.push(corpse);
                }
                if self.combat_opponent == Some(entity) {
                    self.combat_opponent = None;
                }
                if self.local_entity_id() == Some(entity) {
                    self.combat_opponent = None;
                }
                self.npc_animations.remove(&entity);
                self.movement_interp.remove(entity);
                self.entities.retain(|e| e.entity_id != entity);
            }
            ServerMessage::QuestJournal { entries } => {
                self.quest_text = entries;
            }
            ServerMessage::QuestUpdate {
                quest_id,
                stage,
                completed,
            } => {
                let name = format!("Quest {}", quest_id.0);
                let desc = format!(
                    "Stage {stage}{}",
                    if completed { " (complete)" } else { "" }
                );
                if let Some(entry) = self.quest_text.iter_mut().find(|(n, _)| n == &name) {
                    entry.1 = desc.clone();
                } else {
                    self.quest_text.push((name, desc));
                }
            }
            ServerMessage::Dialogue { npc_entity, node } => {
                self.ui.active_dialogue = Some((npc_entity, node));
            }
            ServerMessage::ShopOpen { shop_id, stock } => {
                self.ui.open_shop = Some((shop_id, stock));
            }
            ServerMessage::OpenMarketUpdate { offers } => {
                self.ui.market_offers = offers;
            }
            ServerMessage::TradeUpdate {
                partner,
                their_items,
                your_items,
                ..
            } => {
                self.ui.trade_partner = Some(partner);
                self.ui.trade_their_items = their_items;
                self.ui.trade_your_items = your_items;
            }
            ServerMessage::FriendsUpdate { friends, online } => {
                self.ui.friends = friends;
                self.ui.online_players = online;
            }
            ServerMessage::MinigameStart { minigame_id, wave } => {
                self.ui.status = format!("Minigame {minigame_id} wave {wave}");
            }
            ServerMessage::BossUpdate { boss } => {
                self.ui.status = format!("Boss {} HP {}/{}", boss.name, boss.hp, boss.max_hp);
            }
            ServerMessage::LedgerUpdate {
                rank,
                points,
                active_contract,
            } => {
                self.ui.ledger_rank = rank;
                self.ui.ledger_points = points;
                self.ui.ledger_contract = active_contract;
                if let Some(c) = &self.ui.ledger_contract {
                    self.ui.status = format!(
                        "Ledger rank {rank} ({points} pts): {} left {}",
                        c.name, c.remaining
                    );
                }
            }
            ServerMessage::Error { message } => {
                self.ui.status = message;
            }
            ServerMessage::CollectionLogEntry { item_id, source } => {
                let name = self
                    .content
                    .item(item_id)
                    .map(|i| i.name.clone())
                    .unwrap_or_else(|| format!("Item {}", item_id.0));
                self.ui.collection_log.push((name.clone(), item_id.0));
                self.ui.status = format!("Collected {name} from {source}");
            }
            _ => {}
        }
    }

    fn entity_footprint(&self, entity: &WorldEntity) -> Option<NpcFootprint> {
        match &entity.kind {
            EntityKind::Npc { npc_id, .. } => {
                self.content.npc(*npc_id).map(NpcFootprint::from_def)
            }
            _ => None,
        }
    }

    fn seed_movement_interp(&mut self, _now: Instant) {
        for entity in &self.entities {
            if let Some(tile) = entity_bounds::entity_tile(entity) {
                self.movement_interp.seed_position(
                    entity.entity_id,
                    tile,
                    self.region.as_ref(),
                    self.entity_footprint(entity),
                );
            }
        }
    }

    fn merge_entities(&mut self, entities: Vec<WorldEntity>, now: Instant) {
        let region_id = self.current_region_id;
        let filtered: Vec<_> = entities
            .into_iter()
            .filter(|e| e.region_id == region_id)
            .collect();
        let ids: std::collections::HashSet<_> = filtered.iter().map(|e| e.entity_id).collect();
        for entity in filtered {
            if let Some(tile) = entity_bounds::entity_tile(&entity) {
                self.movement_interp.on_position_change(
                    entity.entity_id,
                    tile,
                    now,
                    self.region.as_ref(),
                    self.entity_footprint(&entity),
                );
            }
            if let Some(existing) = self
                .entities
                .iter_mut()
                .find(|e| e.entity_id == entity.entity_id)
            {
                *existing = entity;
            } else {
                self.entities.push(entity);
            }
        }
        self.entities.retain(|e| ids.contains(&e.entity_id));
        self.movement_interp.prune(&ids);
        self.npc_animations.retain(|id, _| ids.contains(id));
    }

    fn capture_death_corpse(&self, entity_id: openmmo_common::EntityId) -> Option<DeathCorpse> {
        let entity = self.entities.iter().find(|e| e.entity_id == entity_id)?;
        let EntityKind::Npc { npc_id, .. } = &entity.kind else {
            return None;
        };
        if self
            .content
            .npc(*npc_id)
            .and_then(|d| d.model.as_deref())
            .is_none()
        {
            return None;
        }
        let footprint = self.content.npc(*npc_id).map(NpcFootprint::from_def);
        let now = Instant::now();
        let base = self
            .movement_interp
            .visual_center(entity_id, now, self.region.as_ref(), footprint)
            .or_else(|| {
                entity_bounds::entity_tile(entity).map(|tile| {
                    let surface_y = entity_bounds::tile_surface_height(tile, self.region.as_ref());
                    footprint
                        .unwrap_or_default()
                        .world_center(tile, surface_y)
                })
            })?;
        let yaw = self
            .movement_interp
            .visual_facing_yaw(entity_id, now)
            .unwrap_or(0.0);
        let mut player = AnimationPlayer::new(FROG_DEATH);
        player.play(FROG_DEATH, false);
        Some(DeathCorpse {
            npc_id: *npc_id,
            base,
            yaw,
            player,
        })
    }

    fn local_entity_id(&self) -> Option<openmmo_common::EntityId> {
        let lp = self.local_player?;
        self.entities.iter().find_map(|entity| {
            if let EntityKind::Player { player_id, .. } = &entity.kind {
                (*player_id == lp).then_some(entity.entity_id)
            } else {
                None
            }
        })
    }

    fn sync_local_hp(&mut self) {
        if let Some(lp) = self.local_player {
            if let Some(entity) = self.entities.iter().find(
                |e| matches!(&e.kind, EntityKind::Player { player_id, .. } if *player_id == lp),
            ) {
                if let EntityKind::Player { hp, max_hp, .. } = &entity.kind {
                    self.hp = *hp;
                    self.max_hp = *max_hp;
                }
            }
        }
    }

    fn player_id_by_name(&self, name: &str) -> Option<openmmo_common::PlayerId> {
        self.entities.iter().find_map(|entity| {
            if let EntityKind::Player {
                player_id,
                name: player_name,
                ..
            } = &entity.kind
            {
                (player_name == name).then_some(*player_id)
            } else {
                None
            }
        })
    }

    fn close_shop_on_world_interaction(&mut self) {
        self.ui.open_shop = None;
    }

    fn handle_entity_click(&self, entity_id: openmmo_common::EntityId) -> Option<ClientMessage> {
        let entity = self.entities.iter().find(|e| e.entity_id == entity_id)?;
        match &entity.kind {
            EntityKind::Object { .. } => Some(ClientMessage::Scavenge {
                object_entity: entity_id,
            }),
            EntityKind::GroundItem { .. } => Some(ClientMessage::PickupItem {
                ground_entity: entity_id,
            }),
            EntityKind::Npc { aggro_range, .. } => {
                if *aggro_range == 0 {
                    Some(ClientMessage::TalkToNpc {
                        npc_entity: entity_id,
                    })
                } else {
                    Some(ClientMessage::Attack {
                        target: entity_id,
                        style: openmmo_common::CombatStyle::Melee,
                    })
                }
            }
            EntityKind::Boss { .. } => Some(ClientMessage::Attack {
                target: entity_id,
                style: openmmo_common::CombatStyle::Melee,
            }),
            _ => None,
        }
    }

    fn local_player_position(&self) -> Option<openmmo_common::TilePos> {
        let local_id = self.local_player?;
        self.entities.iter().find_map(|entity| {
            if let openmmo_common::EntityKind::Player {
                player_id,
                position,
                ..
            } = &entity.kind
            {
                (*player_id == local_id).then_some(*position)
            } else {
                None
            }
        })
    }

    fn compute_hover(&self, renderer: &Renderer) -> Option<HoverTarget> {
        if !self.ui.connected || self.pointer_over_ui {
            return None;
        }
        let width = renderer.gpu().config.width;
        let height = renderer.gpu().config.height;
        let camera = renderer.camera();
        if let Some(entity_id) = self.input.entity_under_cursor(
            camera,
            width,
            height,
            &self.entities,
            self.region.as_ref(),
            self.local_player,
            &self.content,
        ) {
            return Some(HoverTarget::Entity(entity_id));
        }
        self.input
            .tile_under_cursor(camera, width, height)
            .map(HoverTarget::Tile)
    }

    fn render_frame(
        &mut self,
        egui_ctx: &egui::Context,
        egui_state: &mut EguiWinitState,
        egui_renderer: &mut EguiRenderer,
        renderer: &mut Renderer,
        window: &winit::window::Window,
    ) {
        self.poll_network();

        let now = Instant::now();
        let dt = now.duration_since(self.last_frame).as_secs_f32().min(0.1);
        self.last_frame = now;

        if self.ui.connected {
            if let Some(entity) = self.local_player.and_then(|lp| {
                self.entities.iter().find(
                    |e| matches!(&e.kind, EntityKind::Player { player_id, .. } if *player_id == lp),
                )
            }) {
                if let Some([cx, cy, cz]) = self.movement_interp.visual_center(
                    entity.entity_id,
                    now,
                    self.region.as_ref(),
                    None,
                ) {
                    renderer.camera_mut().center_on_world(cx, cy, cz);
                } else if let Some(position) = entity_bounds::entity_tile(entity) {
                    let surface_y =
                        entity_bounds::tile_surface_height(position, self.region.as_ref());
                    renderer
                        .camera_mut()
                        .center_on_world(
                            position.x as f32 + 0.5,
                            surface_y,
                            position.y as f32 + 0.5,
                        );
                }
            }
        }

        let scroll = self.input.take_scroll();
        if scroll != 0.0 {
            renderer.camera_mut().zoom(scroll);
        }

        let Ok((output, view, mut encoder)) = renderer.begin_frame() else {
            return;
        };

        let hover = self.compute_hover(renderer);

        renderer.render_world_pass(
            &mut encoder,
            &view,
            self.region.as_ref(),
            &self.entities,
            &self.content,
            &mut self.model_cache,
            self.local_player,
            &self.movement_interp,
            hover,
            &mut self.npc_animations,
            &mut self.death_corpses,
            now,
            dt,
        );

        self.death_corpses.retain(|c| !c.player.finished);

        let raw_input = egui_state.take_egui_input(window);

        let mut connect = false;
        let mut ui_action = UiAction::None;

        let full_output = egui_ctx.run(raw_input, |ctx| {
            if !self.ui.connected {
                connect = self.ui.draw_login(ctx);
            } else {
                ui_action = self.ui.draw_hud(
                    ctx,
                    &self.content,
                    &self.inventory,
                    &self.bank,
                    &self.equipment,
                    &self.skills,
                    self.hp,
                    self.max_hp,
                    &self.quest_text,
                    self.combat_opponent,
                    self.local_player_position(),
                    self.region.as_ref(),
                );

                if let Some(opponent) = self.combat_opponent {
                    let mut combatants = Vec::new();
                    if let Some(local_eid) = self.local_entity_id() {
                        combatants.push(local_eid);
                    }
                    combatants.push(opponent);
                    let camera = renderer.camera();
                    let vp = camera
                        .view_projection(renderer.gpu().config.width, renderer.gpu().config.height);
                    draw_combat_health_bars(
                        ctx,
                        &combatants,
                        &self.entities,
                        &self.movement_interp,
                        self.region.as_ref(),
                        &self.content,
                        vp,
                        renderer.gpu().config.width,
                        renderer.gpu().config.height,
                        window.scale_factor() as f32,
                        Instant::now(),
                    );
                }
            }
        });

        if connect {
            if let Some(tx) = &self.net_tx {
                let _ = tx.send(NetCommand::Connect {
                    url: self.ui.connection_url.clone(),
                    username: self.ui.username.clone(),
                    character: self.ui.character_name.clone(),
                    password: self.ui.password.clone(),
                });
            }
            self.ui.status = "Connecting...".into();
        }

        match ui_action {
            UiAction::Chat { channel, message } => {
                self.send(ClientMessage::Chat { channel, message });
            }
            UiAction::DropItem(slot) => {
                self.send(ClientMessage::DropItem { slot, quantity: 1 });
            }
            UiAction::EquipItem(slot) => {
                self.send(ClientMessage::EquipItem { inv_slot: slot });
            }
            UiAction::UnequipItem(slot) => {
                self.send(ClientMessage::UnequipItem { slot });
            }
            UiAction::BankDeposit { inv_slot, quantity } => {
                self.send(ClientMessage::BankDeposit { inv_slot, quantity });
            }
            UiAction::BankWithdraw {
                bank_slot,
                quantity,
            } => {
                self.send(ClientMessage::BankWithdraw {
                    bank_slot,
                    quantity,
                });
            }
            UiAction::Refine { recipe_id } => {
                self.send(ClientMessage::Refine { recipe_id });
            }
            UiAction::CastSpell { target, spell_id } => {
                self.send(ClientMessage::CastSpell { target, spell_id });
            }
            UiAction::SelectSpecialization { skill, branch } => {
                self.send(ClientMessage::SelectSpecialization { skill, branch });
            }
            UiAction::DialogueSelect {
                npc_entity,
                dialogue_id,
                option_index,
            } => {
                self.send(ClientMessage::DialogueSelect {
                    npc_entity,
                    dialogue_id,
                    option_index,
                });
            }
            UiAction::ShopBuy {
                shop_id,
                item_id,
                quantity,
            } => {
                self.send(ClientMessage::ShopBuy {
                    shop_id,
                    item_id,
                    quantity,
                });
            }
            UiAction::MarketPlaceOffer {
                item_id,
                quantity,
                price_per,
                is_buy,
            } => {
                self.send(ClientMessage::MarketPlaceOffer {
                    item_id,
                    quantity,
                    price_per,
                    is_buy,
                });
            }
            UiAction::MarketCancelOffer { offer_id } => {
                self.send(ClientMessage::MarketCancelOffer { offer_id });
            }
            UiAction::FriendAdd { name } => {
                self.send(ClientMessage::FriendAdd { name });
            }
            UiAction::PrivateMessage { to, message } => {
                self.send(ClientMessage::PrivateMessage { to, message });
            }
            UiAction::TradeRequestByName { name } => {
                if let Some(target) = self.player_id_by_name(&name) {
                    self.send(ClientMessage::TradeRequest {
                        target_player: target,
                    });
                } else {
                    self.ui.status = format!("Player '{name}' not found nearby");
                }
            }
            UiAction::TradeOffer { items } => {
                if let Some(name) = self.ui.trade_partner.clone() {
                    if let Some(target) = self.player_id_by_name(&name) {
                        self.send(ClientMessage::TradeOffer { target, items });
                    } else {
                        self.ui.status = format!("Trade partner '{name}' not found");
                    }
                }
            }
            UiAction::TradeAccept => {
                if let Some(name) = self.ui.trade_partner.clone() {
                    if let Some(target) = self.player_id_by_name(&name) {
                        self.send(ClientMessage::TradeAccept { target });
                    } else {
                        self.ui.status = format!("Trade partner '{name}' not found");
                    }
                }
            }
            UiAction::JoinMinigame { minigame_id } => {
                self.send(ClientMessage::JoinMinigame { minigame_id });
            }
            UiAction::ContextMenu(menu_action) => {
                self.close_shop_on_world_interaction();
                self.send(to_client_message(menu_action));
            }
            UiAction::Connect => {}
            UiAction::None => {}
        }

        let pixels_per_point = window.scale_factor() as f32;

        if self.ui.connected && self.input.right_clicked && !egui_ctx.wants_pointer_input() {
            self.input.right_clicked = false;
            let width = renderer.gpu().config.width;
            let height = renderer.gpu().config.height;
            let tile = self
                .input
                .tile_under_cursor(renderer.camera(), width, height);
            if let Some(tile) = tile {
                let entity_id = self.input.entity_under_cursor(
                    renderer.camera(),
                    width,
                    height,
                    &self.entities,
                    self.region.as_ref(),
                    self.local_player,
                    &self.content,
                );
                let screen_pos = egui::pos2(
                    self.input.mouse_x / pixels_per_point,
                    self.input.mouse_y / pixels_per_point,
                );
                self.ui.context_menu = build_context_menu(
                    &self.content,
                    self.region.as_ref(),
                    &self.entities,
                    self.local_player,
                    entity_id,
                    tile,
                    screen_pos,
                );
            }
        } else if self.input.right_clicked {
            self.input.right_clicked = false;
        }

        if self.ui.connected && self.input.left_clicked {
            self.input.left_clicked = false;
            self.ui.context_menu = None;
            let width = renderer.gpu().config.width;
            let height = renderer.gpu().config.height;
            let tile = self
                .input
                .tile_under_cursor(renderer.camera(), width, height);
            if let Some(entity_id) = self.input.entity_under_cursor(
                renderer.camera(),
                width,
                height,
                &self.entities,
                self.region.as_ref(),
                self.local_player,
                &self.content,
            ) {
                if let Some(msg) = self.handle_entity_click(entity_id) {
                    self.close_shop_on_world_interaction();
                    self.send(msg);
                } else if let Some(target) = tile {
                    if self.local_player_position() != Some(target) {
                        self.close_shop_on_world_interaction();
                        self.send(ClientMessage::WalkIntent { target });
                    }
                }
            } else if let Some(target) = tile {
                if self.local_player_position() != Some(target) {
                    self.close_shop_on_world_interaction();
                    self.send(ClientMessage::WalkIntent { target });
                }
            }
        }

        egui_state.handle_platform_output(window, full_output.platform_output);
        self.pointer_over_ui = egui_ctx.wants_pointer_input();

        let screen_desc = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [renderer.gpu().config.width, renderer.gpu().config.height],
            pixels_per_point,
        };

        let paint_jobs = egui_ctx.tessellate(full_output.shapes, pixels_per_point);
        for (id, image_delta) in &full_output.textures_delta.set {
            egui_renderer.update_texture(
                &renderer.gpu().device,
                &renderer.gpu().queue,
                *id,
                image_delta,
            );
        }
        egui_renderer.update_buffers(
            &renderer.gpu().device,
            &renderer.gpu().queue,
            &mut encoder,
            &paint_jobs,
            &screen_desc,
        );

        {
            let render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Egui Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });
            let mut render_pass = render_pass.forget_lifetime();
            egui_renderer.render(&mut render_pass, &paint_jobs, &screen_desc);
        }

        for id in &full_output.textures_delta.free {
            egui_renderer.free_texture(id);
        }

        renderer.finish_frame(encoder, output);
    }
}
