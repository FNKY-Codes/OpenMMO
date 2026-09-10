//! In-game UI (egui), laid out like classic RuneScape: minimap and HP orb
//! top-right, icon-tabbed side panel bottom-right, chat bottom-left, hover
//! text top-left, and windows for the bank, shops, stations and dialogue.

pub mod chat;
mod context_menu;
pub mod grid;
pub mod icons;
pub mod minimap;
pub mod panels;
mod theme;
mod widgets;
pub mod windows;

use std::time::Instant;

use egui::{Context, RichText, Vec2};
use openmmo_common::{
    ContentPack, DialogueNode, EntityId, EntityKind, Equipment, Inventory, ItemId, RegionDef,
    SkillBook, StationTag, TilePos, WorldEntity,
};
use openmmo_protocol::{ChatChannel, LedgerContract};

use crate::entity_bounds;
use crate::math::{self, Vec3};
use crate::movement_interp::EntityMovementInterp;

pub use chat::{ChatKind, ChatOutput, ChatState};
pub use context_menu::{
    build_context_menu, examine_text, harvest_verb, primary_action, to_client_message, ContextMenu,
    ContextMenuAction,
};
pub use panels::PanelTab;
pub use theme::setup_theme;
pub use widgets::entity_display_name;

pub struct GameUi {
    // Login
    pub connection_url: String,
    pub username: String,
    pub character_name: String,
    pub password: String,
    pub connected: bool,
    pub status: String,

    // Layout / HUD
    pub active_tab: PanelTab,
    pub show_minimap: bool,
    pub show_hover_text: bool,
    pub minimap_cache: Option<minimap::MinimapCache>,
    pub chat: ChatState,
    pub xp_drops: Vec<(String, u64, Instant)>,
    pub hover_text: Option<String>,
    pub context_menu: Option<ContextMenu>,

    // Windows
    pub active_dialogue: Option<(EntityId, DialogueNode)>,
    pub open_shop: Option<(String, Vec<openmmo_common::ShopStock>)>,
    pub open_station: Option<(StationTag, TilePos)>,
    /// Recipe being made repeatedly and how many are left to start.
    pub craft_queue: Option<(String, u32)>,

    // Market
    pub market_offers: Vec<openmmo_common::MarketOffer>,
    pub market_item_id: ItemId,
    pub market_selected_inv_slot: Option<usize>,
    pub market_qty: String,
    pub market_price: String,
    pub market_is_buy: bool,

    // Social
    pub friends: Vec<String>,
    pub online_players: Vec<String>,
    pub friend_add_input: String,
    pub trade_partner: Option<String>,
    pub trade_their_items: Vec<openmmo_common::InventorySlot>,
    pub trade_your_items: Vec<openmmo_common::InventorySlot>,
    pub trade_pending_items: Vec<openmmo_common::InventorySlot>,
    pub trade_partner_name: String,

    // Progress
    pub ledger_rank: u32,
    pub ledger_points: u32,
    pub ledger_contract: Option<LedgerContract>,
    pub collection_log: Vec<(String, u32)>,
}

impl Default for GameUi {
    fn default() -> Self {
        Self {
            connection_url: "ws://127.0.0.1:8080/ws".to_string(),
            username: "player".to_string(),
            character_name: "Adventurer".to_string(),
            password: String::new(),
            connected: false,
            status: "Disconnected".to_string(),
            active_tab: PanelTab::Inventory,
            show_minimap: true,
            show_hover_text: true,
            minimap_cache: None,
            chat: ChatState::default(),
            xp_drops: Vec::new(),
            hover_text: None,
            context_menu: None,
            active_dialogue: None,
            open_shop: None,
            open_station: None,
            craft_queue: None,
            market_offers: Vec::new(),
            market_item_id: ItemId(2),
            market_selected_inv_slot: None,
            market_qty: "1".to_string(),
            market_price: "10".to_string(),
            market_is_buy: false,
            friends: Vec::new(),
            online_players: Vec::new(),
            friend_add_input: String::new(),
            trade_partner: None,
            trade_their_items: Vec::new(),
            trade_your_items: Vec::new(),
            trade_pending_items: Vec::new(),
            trade_partner_name: String::new(),
            ledger_rank: 0,
            ledger_points: 0,
            ledger_contract: None,
            collection_log: Vec::new(),
        }
    }
}

/// Text shown top-left for whatever is under the cursor, RuneScape style.
pub fn hover_label(
    content: &ContentPack,
    entities: &[WorldEntity],
    hover: Option<crate::renderer::HoverTarget>,
) -> Option<String> {
    use crate::renderer::HoverTarget;
    match hover? {
        HoverTarget::Tile(_) => Some("Walk here".into()),
        HoverTarget::Entity(id) => {
            let entity = entities.iter().find(|e| e.entity_id == id)?;
            let (name, subtitle) = widgets::entity_display_name(content, entity);
            let verb = primary_action(content, entity).map(|(v, _)| v);
            let level = match &entity.kind {
                EntityKind::Npc { .. } | EntityKind::Boss { .. } => subtitle
                    .filter(|s| s.starts_with("Level"))
                    .map(|s| format!(" ({})", s.to_lowercase()))
                    .unwrap_or_default(),
                _ => String::new(),
            };
            Some(match verb {
                Some(v) => format!("{v} {name}{level}"),
                None => format!("Examine {name}{level}"),
            })
        }
    }
}

impl GameUi {
    pub fn draw_login(&mut self, ctx: &Context) -> bool {
        let mut connect = false;
        egui::Window::new(RichText::new("OpenMMO").color(theme::ACCENT).size(22.0))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.set_width(300.0);
                ui.label(RichText::new("Verdant Reach awaits.").color(theme::TEXT_MUTED));
                ui.separator();
                ui.label("Server");
                ui.text_edit_singleline(&mut self.connection_url);
                ui.label("Username");
                ui.text_edit_singleline(&mut self.username);
                ui.label("Character name");
                ui.text_edit_singleline(&mut self.character_name);
                ui.label("Password");
                let pw = ui.add(egui::TextEdit::singleline(&mut self.password).password(true));
                ui.separator();
                ui.label(RichText::new(&self.status).color(theme::TEXT_MUTED).small());
                ui.label(
                    RichText::new("New username? The account is created with this password.")
                        .color(theme::TEXT_MUTED)
                        .small(),
                );
                let enter = pw.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                if ui
                    .add(egui::Button::new("Connect").min_size(Vec2::new(120.0, 26.0)))
                    .clicked()
                    || enter
                {
                    connect = true;
                }
            });
        connect
    }

    #[allow(clippy::too_many_arguments)]
    pub fn draw_hud(
        &mut self,
        ctx: &Context,
        content: &ContentPack,
        inventory: &Inventory,
        bank: &Inventory,
        equipment: &Equipment,
        skills: &SkillBook,
        hp: u32,
        max_hp: u32,
        quest_text: &[(String, String)],
        combat_opponent: Option<EntityId>,
        player_tile: Option<TilePos>,
        region: Option<&RegionDef>,
        entities: &[WorldEntity],
        local_player: Option<openmmo_common::PlayerId>,
    ) -> UiAction {
        let mut action = UiAction::None;
        let now = Instant::now();

        // Hover text, top-left.
        if self.show_hover_text {
            if let Some(text) = &self.hover_text {
                egui::Area::new(egui::Id::new("hover_text"))
                    .anchor(egui::Align2::LEFT_TOP, Vec2::new(10.0, 8.0))
                    .order(egui::Order::Foreground)
                    .interactable(false)
                    .show(ctx, |ui| {
                        ui.label(
                            RichText::new(text)
                                .color(theme::HOVER_TEXT)
                                .size(15.0)
                                .background_color(egui::Color32::from_black_alpha(120)),
                        );
                    });
            }
        }

        // XP drops float up beside the minimap.
        self.xp_drops
            .retain(|(_, _, at)| now.duration_since(*at).as_secs_f32() < 1.6);
        for (i, (skill, amount, at)) in self.xp_drops.iter().enumerate() {
            let t = now.duration_since(*at).as_secs_f32();
            egui::Area::new(egui::Id::new(("xp_drop", i)))
                .anchor(
                    egui::Align2::RIGHT_TOP,
                    Vec2::new(-250.0, 40.0 + i as f32 * 18.0 - t * 30.0),
                )
                .order(egui::Order::Foreground)
                .interactable(false)
                .show(ctx, |ui| {
                    ui.label(
                        RichText::new(format!("+{amount} {skill}"))
                            .color(theme::ACCENT)
                            .size(14.0)
                            .background_color(egui::Color32::from_black_alpha(110)),
                    );
                });
        }

        if self.show_minimap {
            let input = minimap::MinimapInput {
                region,
                content,
                entities,
                local_player,
                player_tile,
                hp,
                max_hp,
            };
            if let Some(target) = minimap::draw_minimap(ctx, &mut self.minimap_cache, &input) {
                action = UiAction::WalkTo(target);
            }
        }

        let data = panels::PanelData {
            content,
            inventory,
            equipment,
            skills,
            quest_text,
            combat_opponent,
        };
        let panel_action = panels::draw_side_panel(ctx, self, &data);
        if !matches!(panel_action, UiAction::None) {
            action = panel_action;
        }

        match chat::draw_chat(ctx, &mut self.chat) {
            Some(ChatOutput::Say(channel, message)) => {
                action = UiAction::Chat { channel, message };
            }
            Some(ChatOutput::Whisper(to, message)) => {
                action = UiAction::PrivateMessage { to, message };
            }
            None => {}
        }

        // Windows
        fn take(a: UiAction, action: &mut UiAction) {
            if !matches!(a, UiAction::None) {
                *action = a;
            }
        }
        take(windows::draw_dialogue(ctx, self, content), &mut action);
        take(
            windows::draw_shop(ctx, self, content, inventory),
            &mut action,
        );
        take(
            windows::draw_trade(ctx, self, content, inventory),
            &mut action,
        );
        match self.open_station {
            Some((StationTag::Bank, _)) => take(
                windows::draw_bank(ctx, self, content, inventory, bank),
                &mut action,
            ),
            Some((StationTag::Market, _)) => take(
                windows::draw_market(ctx, self, content, inventory),
                &mut action,
            ),
            Some(_) => take(
                windows::draw_crafting(ctx, self, content, inventory, skills),
                &mut action,
            ),
            None => {}
        }

        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.context_menu = None;
        }
        if let Some(menu) = self.context_menu.clone() {
            if let Some(menu_action) = context_menu::draw_context_menu(ctx, &menu) {
                self.context_menu = None;
                action = UiAction::ContextMenu(menu_action);
            }
        }

        action
    }
}

#[allow(clippy::too_many_arguments)]
pub fn draw_combat_health_bars(
    ctx: &Context,
    entity_ids: &[EntityId],
    entities: &[WorldEntity],
    movement_interp: &EntityMovementInterp,
    region: Option<&RegionDef>,
    content: &ContentPack,
    view_proj: math::Mat4,
    width: u32,
    height: u32,
    pixels_per_point: f32,
    now: Instant,
) {
    let width_f = width as f32;
    let height_f = height as f32;
    let bar_width = 52.0;

    for entity_id in entity_ids {
        let Some(entity) = entities.iter().find(|e| e.entity_id == *entity_id) else {
            continue;
        };
        let Some((hp, max_hp)) = entity_hp(entity) else {
            continue;
        };
        let Some(anchor) = entity_health_anchor(entity, movement_interp, region, content, now)
        else {
            continue;
        };
        let Some((screen_x, screen_y)) =
            math::world_to_screen(anchor, view_proj, width_f, height_f)
        else {
            continue;
        };

        let pos = egui::pos2(
            screen_x / pixels_per_point - bar_width * 0.5,
            screen_y / pixels_per_point - 10.0,
        );
        egui::Area::new(egui::Id::new(("combat_hp", entity_id.0)))
            .order(egui::Order::Foreground)
            .fixed_pos(pos)
            .show(ctx, |ui| {
                widgets::world_hp_bar(ui, hp, max_hp);
            });
    }
}

fn entity_hp(entity: &WorldEntity) -> Option<(u32, u32)> {
    match &entity.kind {
        EntityKind::Player { hp, max_hp, .. }
        | EntityKind::Npc { hp, max_hp, .. }
        | EntityKind::Boss { hp, max_hp, .. } => Some((*hp, *max_hp)),
        _ => None,
    }
}

fn entity_health_anchor(
    entity: &WorldEntity,
    movement_interp: &EntityMovementInterp,
    region: Option<&RegionDef>,
    content: &ContentPack,
    now: Instant,
) -> Option<Vec3> {
    let (_, height) = entity_bounds::entity_cube_dims(&entity.kind);
    let footprint = match &entity.kind {
        EntityKind::Npc { npc_id, .. } => content
            .npc(*npc_id)
            .map(openmmo_common::NpcFootprint::from_def),
        _ => None,
    };
    let [cx, surface_y, cz] =
        movement_interp.visual_center(entity.entity_id, now, region, footprint)?;
    Some(Vec3::new(cx, surface_y + height + 0.15, cz))
}

#[derive(Debug, Clone)]
pub enum UiAction {
    None,
    Chat {
        channel: ChatChannel,
        message: String,
    },
    WalkTo(TilePos),
    DropItem(usize),
    EquipItem(usize),
    UnequipItem(openmmo_common::EquipSlot),
    UseItem(usize),
    BankDeposit {
        inv_slot: usize,
        quantity: u32,
    },
    BankWithdraw {
        bank_slot: usize,
        quantity: u32,
    },
    Refine {
        recipe_id: String,
    },
    Craft {
        recipe_id: String,
        count: u32,
    },
    DialogueSelect {
        npc_entity: EntityId,
        dialogue_id: String,
        option_index: usize,
    },
    CastSpell {
        target: EntityId,
        spell_id: String,
    },
    SelectSpecialization {
        skill: openmmo_common::Skill,
        branch: String,
    },
    ShopBuy {
        shop_id: String,
        item_id: ItemId,
        quantity: u32,
    },
    ShopSell {
        shop_id: String,
        inv_slot: usize,
        quantity: u32,
    },
    MarketPlaceOffer {
        item_id: ItemId,
        quantity: u32,
        price_per: u32,
        is_buy: bool,
    },
    MarketCancelOffer {
        offer_id: uuid::Uuid,
    },
    FriendAdd {
        name: String,
    },
    PrivateMessage {
        to: String,
        message: String,
    },
    TradeRequestByName {
        name: String,
    },
    TradeOffer {
        items: Vec<openmmo_common::InventorySlot>,
    },
    TradeAccept,
    JoinMinigame {
        minigame_id: String,
    },
    ContextMenu(ContextMenuAction),
    Connect,
}
