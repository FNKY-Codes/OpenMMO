mod side_panel;
mod theme;
mod widgets;

use egui::{Context, RichText};
use openmmo_common::{
    ContentPack, DialogueNode, Equipment, Inventory, ItemId, SkillBook,
};
use openmmo_protocol::LedgerContract;

pub use theme::setup_theme;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidePanelTab {
    Inventory,
    Bank,
    Skills,
    Quests,
    Market,
    Friends,
}

pub struct GameUi {
    pub active_tab: Option<SidePanelTab>,
    pub show_minimap: bool,
    pub chat_input: String,
    pub chat_log: Vec<(String, String)>,
    pub combat_log: Vec<String>,
    pub xp_drops: Vec<(String, u64)>,
    pub connection_url: String,
    pub username: String,
    pub character_name: String,
    pub connected: bool,
    pub status: String,
    pub active_dialogue: Option<(openmmo_common::EntityId, DialogueNode)>,
    pub open_shop: Option<(String, Vec<openmmo_common::ShopStock>)>,
    pub bank_mode_deposit: bool,
    pub market_offers: Vec<openmmo_common::MarketOffer>,
    pub market_item_id: ItemId,
    pub market_selected_inv_slot: Option<usize>,
    pub market_qty: String,
    pub market_price: String,
    pub market_is_buy: bool,
    pub friends: Vec<String>,
    pub online_players: Vec<String>,
    pub friend_add_input: String,
    pub pm_to: String,
    pub pm_message: String,
    pub trade_partner: Option<String>,
    pub trade_their_items: Vec<openmmo_common::InventorySlot>,
    pub trade_your_items: Vec<openmmo_common::InventorySlot>,
    pub trade_pending_items: Vec<openmmo_common::InventorySlot>,
    pub trade_partner_name: String,
    pub ledger_rank: u32,
    pub ledger_points: u32,
    pub ledger_contract: Option<LedgerContract>,
}

impl Default for GameUi {
    fn default() -> Self {
        Self {
            active_tab: Some(SidePanelTab::Inventory),
            show_minimap: true,
            chat_input: String::new(),
            chat_log: Vec::new(),
            combat_log: Vec::new(),
            xp_drops: Vec::new(),
            connection_url: "ws://127.0.0.1:8080/ws".to_string(),
            username: "player".to_string(),
            character_name: "Adventurer".to_string(),
            connected: false,
            status: "Disconnected".to_string(),
            active_dialogue: None,
            open_shop: None,
            bank_mode_deposit: true,
            market_offers: Vec::new(),
            market_item_id: ItemId(2),
            market_selected_inv_slot: None,
            market_qty: "1".to_string(),
            market_price: "10".to_string(),
            market_is_buy: false,
            friends: Vec::new(),
            online_players: Vec::new(),
            friend_add_input: String::new(),
            pm_to: String::new(),
            pm_message: String::new(),
            trade_partner: None,
            trade_their_items: Vec::new(),
            trade_your_items: Vec::new(),
            trade_pending_items: Vec::new(),
            trade_partner_name: String::new(),
            ledger_rank: 0,
            ledger_points: 0,
            ledger_contract: None,
        }
    }
}

impl GameUi {
    pub fn draw_login(&mut self, ctx: &Context) -> bool {
        let mut connect = false;
        egui::Window::new("OpenMMO Login")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label(RichText::new("OpenMMO").size(24.0));
                ui.separator();
                ui.label("Server URL");
                ui.text_edit_singleline(&mut self.connection_url);
                ui.label("Username");
                ui.text_edit_singleline(&mut self.username);
                ui.label("Character Name");
                ui.text_edit_singleline(&mut self.character_name);
                ui.separator();
                ui.label(&self.status);
                if ui.button("Connect").clicked() {
                    connect = true;
                }
            });
        connect
    }

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
    ) -> UiAction {
        let mut action = UiAction::None;

        for (skill, amount) in &self.xp_drops.clone() {
            egui::Area::new(egui::Id::new(format!("xp_{skill}_{amount}")))
                .anchor(egui::Align2::CENTER_TOP, [0.0, 40.0])
                .show(ctx, |ui| {
                    ui.label(
                        RichText::new(format!("+{amount} {skill} XP"))
                            .color(theme::ACCENT),
                    );
                });
        }
        self.xp_drops.clear();

        if self.show_minimap {
            egui::Window::new("Minimap")
                .default_pos([10.0, 10.0])
                .resizable(false)
                .show(ctx, |ui| {
                    let (rect, _) =
                        ui.allocate_exact_size(egui::vec2(120.0, 120.0), egui::Sense::hover());
                    ui.painter()
                        .rect_filled(rect, 4.0, egui::Color32::from_rgb(20, 40, 20));
                    ui.painter()
                        .circle_filled(rect.center(), 4.0, theme::ACCENT);
                });
        }

        egui::TopBottomPanel::bottom("chat_panel").show(ctx, |ui| {
            egui::ScrollArea::vertical()
                .max_height(80.0)
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    for line in &self.combat_log {
                        ui.colored_label(egui::Color32::LIGHT_RED, line);
                    }
                    for (from, msg) in &self.chat_log {
                        ui.label(format!("[{from}] {msg}"));
                    }
                });
            ui.horizontal(|ui| {
                let response = ui.text_edit_singleline(&mut self.chat_input);
                if (response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))
                    || ui.button("Send").clicked()
                {
                    if !self.chat_input.is_empty() {
                        action = UiAction::Chat(self.chat_input.clone());
                        self.chat_input.clear();
                    }
                }
            });
        });

        egui::SidePanel::right("side_panel")
            .default_width(280.0)
            .show(ctx, |ui| {
                widgets::hp_bar(ui, hp, max_hp);
                widgets::status_row(ui, self.connected, &self.status);

                ui.horizontal(|ui| {
                    ui.checkbox(&mut self.show_minimap, "Map");
                });
                ui.separator();

                widgets::tab_bar(ui, &mut self.active_tab);

                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let tab_action = side_panel::draw_active_tab(
                            ui,
                            self,
                            content,
                            inventory,
                            bank,
                            equipment,
                            skills,
                            quest_text,
                        );
                        if !matches!(tab_action, UiAction::None) {
                            action = tab_action;
                        }

                        let trade_action =
                            side_panel::draw_trade_section(ui, self, content, inventory);
                        if !matches!(trade_action, UiAction::None) {
                            action = trade_action;
                        }

                        let footer_action = side_panel::draw_activities_footer(ui);
                        if !matches!(footer_action, UiAction::None) {
                            action = footer_action;
                        }
                    });
            });

        if let Some((npc_entity, node)) = self.active_dialogue.clone() {
            egui::Window::new("Dialogue")
                .collapsible(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label(&node.text);
                    ui.separator();
                    for (idx, opt) in node.options.iter().enumerate() {
                        if ui.button(&opt.label).clicked() {
                            action = UiAction::DialogueSelect {
                                npc_entity,
                                option_index: idx,
                            };
                            self.active_dialogue = None;
                        }
                    }
                });
        }

        if let Some((shop_id, stock)) = self.open_shop.clone() {
            egui::Window::new("Shop")
                .collapsible(false)
                .show(ctx, |ui| {
                    ui.label(format!("Shop: {shop_id}"));
                    for item in &stock {
                        let name = widgets::item_name(content, item.item_id);
                        if ui
                            .button(format!(
                                "Buy {name} x{} for {}",
                                item.quantity, item.price
                            ))
                            .clicked()
                        {
                            action = UiAction::ShopBuy {
                                shop_id: shop_id.clone(),
                                item_id: item.item_id,
                                quantity: 1,
                            };
                        }
                    }
                    if ui.button("Close").clicked() {
                        self.open_shop = None;
                    }
                });
        }

        action
    }
}

#[derive(Debug, Clone)]
pub enum UiAction {
    None,
    Chat(String),
    DropItem(usize),
    EquipItem(usize),
    UnequipItem(openmmo_common::EquipSlot),
    BankDeposit { inv_slot: usize, quantity: u32 },
    BankWithdraw { bank_slot: usize, quantity: u32 },
    Refine { recipe_id: String },
    DialogueSelect {
        npc_entity: openmmo_common::EntityId,
        option_index: usize,
    },
    ShopBuy {
        shop_id: String,
        item_id: ItemId,
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
    Connect,
}
