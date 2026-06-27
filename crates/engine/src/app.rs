use std::sync::mpsc::{Receiver, Sender};

use egui_wgpu::wgpu;
use egui_wgpu::Renderer as EguiRenderer;
use egui_winit::winit;
use egui_winit::State as EguiWinitState;
use openmmo_common::{ContentPack, EntityKind, Equipment, RegionDef, WorldEntity};
use openmmo_protocol::{ClientMessage, ServerMessage};

use egui_winit::egui;

use crate::{
    input::InputState,
    renderer::Renderer,
    ui::{build_context_menu, setup_theme, to_client_message, GameUi, UiAction},
};

#[derive(Debug)]
pub enum NetCommand {
    Connect {
        url: String,
        username: String,
        character: String,
    },
    Send(ClientMessage),
}

pub struct EngineApp {
    input: InputState,
    pub ui: GameUi,
    pub content: ContentPack,
    pub entities: Vec<WorldEntity>,
    pub region: Option<RegionDef>,
    pub local_player: Option<openmmo_common::PlayerId>,
    pub inventory: openmmo_common::Inventory,
    pub bank: openmmo_common::Inventory,
    pub equipment: Equipment,
    pub skills: openmmo_common::SkillBook,
    pub hp: u32,
    pub max_hp: u32,
    pub quest_text: Vec<(String, String)>,
    pub net_tx: Option<Sender<NetCommand>>,
    pub net_rx: Option<Receiver<ServerMessage>>,
}

impl Default for EngineApp {
    fn default() -> Self {
        Self {
            input: InputState::default(),
            ui: GameUi::default(),
            content: ContentPack::default(),
            entities: Vec::new(),
            region: None,
            local_player: None,
            inventory: openmmo_common::Inventory::new(openmmo_common::INVENTORY_SIZE),
            bank: openmmo_common::Inventory::new(openmmo_common::BANK_SIZE),
            equipment: Equipment::default(),
            skills: openmmo_common::SkillBook::new_mvp(),
            hp: 10,
            max_hp: 10,
            quest_text: Vec::new(),
            net_tx: None,
            net_rx: None,
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
        let mut renderer = pollster::block_on(Renderer::new(window.clone()));
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

    fn send(&self, msg: ClientMessage) {
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
                ..
            } => {
                self.merge_entities(entities);
                if let Some(lp) = local_player {
                    self.local_player = Some(lp);
                }
                self.sync_local_hp();
            }
            ServerMessage::StateDelta { entities, .. } => {
                self.merge_entities(entities);
                self.sync_local_hp();
            }
            ServerMessage::PlayerUpdate {
                player_id,
                position,
                hp,
                max_hp,
                ..
            } => {
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
                self.ui
                    .xp_drops
                    .push((skill.name().to_string(), amount));
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
                self.sync_local_hp();
            }
            ServerMessage::Death { entity, .. } => {
                self.ui.combat_log.push(format!("Entity {} died", entity.0));
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
                let desc = format!("Stage {stage}{}", if completed { " (complete)" } else { "" });
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
                    self.ui.status =
                        format!("Ledger rank {rank} ({points} pts): {} left {}", c.name, c.remaining);
                }
            }
            ServerMessage::Error { message } => {
                self.ui.status = message;
            }
            _ => {}
        }
    }

    fn merge_entities(&mut self, entities: Vec<WorldEntity>) {
        let ids: std::collections::HashSet<_> = entities.iter().map(|e| e.entity_id).collect();
        for entity in entities {
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
    }

    fn sync_local_hp(&mut self) {
        if let Some(lp) = self.local_player {
            if let Some(entity) = self.entities.iter().find(|e| {
                matches!(&e.kind, EntityKind::Player { player_id, .. } if *player_id == lp)
            }) {
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

    fn handle_entity_click(&self, entity_id: openmmo_common::EntityId) -> Option<ClientMessage> {
        let entity = self.entities.iter().find(|e| e.entity_id == entity_id)?;
        match &entity.kind {
            EntityKind::Object { .. } => Some(ClientMessage::Scavenge {
                object_entity: entity_id,
            }),
            EntityKind::GroundItem { .. } => Some(ClientMessage::PickupItem {
                ground_entity: entity_id,
            }),
            EntityKind::Npc {
                aggro_range,
                ..
            } => {
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

    fn render_frame(
        &mut self,
        egui_ctx: &egui::Context,
        egui_state: &mut EguiWinitState,
        egui_renderer: &mut EguiRenderer,
        renderer: &mut Renderer,
        window: &winit::window::Window,
    ) {
        self.poll_network();

        if self.ui.connected {
            if let Some(position) = self.local_player_position() {
                renderer.camera_mut().center_on_tile(position);
            }
        }

        let scroll = self.input.take_scroll();
        if scroll != 0.0 {
            renderer.camera_mut().zoom(scroll);
        }

        let Ok((output, view, mut encoder)) = renderer.begin_frame() else {
            return;
        };

        renderer.render_world_pass(
            &mut encoder,
            &view,
            self.region.as_ref(),
            &self.entities,
            self.local_player,
        );

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
                );
            }
        });

        if connect {
            if let Some(tx) = &self.net_tx {
                let _ = tx.send(NetCommand::Connect {
                    url: self.ui.connection_url.clone(),
                    username: self.ui.username.clone(),
                    character: self.ui.character_name.clone(),
                });
            }
            self.ui.status = "Connecting...".into();
        }

        match ui_action {
            UiAction::Chat(msg) => {
                self.send(ClientMessage::Chat {
                    channel: openmmo_protocol::ChatChannel::Local,
                    message: msg,
                });
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
            UiAction::BankWithdraw { bank_slot, quantity } => {
                self.send(ClientMessage::BankWithdraw {
                    bank_slot,
                    quantity,
                });
            }
            UiAction::Refine { recipe_id } => {
                self.send(ClientMessage::Refine { recipe_id });
            }
            UiAction::DialogueSelect {
                npc_entity,
                option_index,
            } => {
                self.send(ClientMessage::DialogueSelect {
                    npc_entity,
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
                );
                let screen_pos =
                    egui::pos2(self.input.mouse_x / pixels_per_point, self.input.mouse_y / pixels_per_point);
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
            ) {
                if let Some(msg) = self.handle_entity_click(entity_id) {
                    self.send(msg);
                } else if let Some(target) = tile {
                    if self.local_player_position() != Some(target) {
                        self.send(ClientMessage::WalkIntent { target });
                    }
                }
            } else if let Some(target) = tile {
                if self.local_player_position() != Some(target) {
                    self.send(ClientMessage::WalkIntent { target });
                }
            }
        }

        egui_state.handle_platform_output(window, full_output.platform_output);

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
