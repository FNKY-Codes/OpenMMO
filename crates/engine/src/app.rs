use std::sync::mpsc::{Receiver, Sender};

use egui_wgpu::wgpu;
use egui_wgpu::Renderer as EguiRenderer;
use egui_winit::State as EguiWinitState;
use egui_winit::winit;
use openmmo_common::{RegionDef, WorldEntity};
use openmmo_protocol::{ClientMessage, ServerMessage};

use egui_winit::egui;

use crate::{
    input::InputState,
    renderer::Renderer,
    ui::{GameUi, UiAction},
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
    pub entities: Vec<WorldEntity>,
    pub region: Option<RegionDef>,
    pub local_player: Option<openmmo_common::PlayerId>,
    pub inventory: openmmo_common::Inventory,
    pub bank: openmmo_common::Inventory,
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
            entities: Vec::new(),
            region: None,
            local_player: None,
            inventory: openmmo_common::Inventory::new(openmmo_common::INVENTORY_SIZE),
            bank: openmmo_common::Inventory::new(openmmo_common::BANK_SIZE),
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

                            if !response.consumed && self.input.right_dragging {
                                let (dx, dy) = self.input.drag_delta();
                                renderer
                                    .camera_mut()
                                    .rotate(-dx * 0.005, dy * 0.005);
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
                                        self.input.right_dragging = true;
                                        self.input.begin_drag();
                                    }
                                    (
                                        winit::event::ElementState::Released,
                                        winit::event::MouseButton::Right,
                                    ) => {
                                        self.input.right_dragging = false;
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
                self.entities = entities;
                if let Some(lp) = local_player {
                    self.local_player = Some(lp);
                }
                if let Some(lp) = self.local_player {
                    if let Some(entity) = self.entities.iter().find(|e| {
                        matches!(&e.kind, openmmo_common::EntityKind::Player { player_id, .. } if *player_id == lp)
                    }) {
                        if let openmmo_common::EntityKind::Player { hp, max_hp, .. } =
                            &entity.kind
                        {
                            self.hp = *hp;
                            self.max_hp = *max_hp;
                        }
                    }
                }
            }
            ServerMessage::StateDelta { entities, .. } => {
                self.entities = entities;
            }
            ServerMessage::InventoryUpdate {
                inventory,
                bank,
                ..
            } => {
                self.inventory = inventory;
                self.bank = bank;
            }
            ServerMessage::SkillUpdate { skills, .. } => {
                self.skills = skills;
            }
            ServerMessage::ChatMessage { from, message, .. } => {
                self.ui.chat_log.push((from, message));
            }
            ServerMessage::Error { message } => {
                self.ui.status = message;
            }
            _ => {}
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
                    &self.inventory,
                    &self.bank,
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
                self.send(ClientMessage::DropItem {
                    slot,
                    quantity: 1,
                });
            }
            UiAction::Connect => {}
            UiAction::None => {}
        }

        if self.ui.connected && self.input.left_clicked {
            self.input.left_clicked = false;
            if let Some(tile) = self.input.tile_under_cursor(
                renderer.camera(),
                renderer.gpu().config.width,
                renderer.gpu().config.height,
            ) {
                self.send(ClientMessage::WalkIntent { target: tile });
            }
        }

        egui_state.handle_platform_output(window, full_output.platform_output);

        let pixels_per_point = window.scale_factor() as f32;
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
