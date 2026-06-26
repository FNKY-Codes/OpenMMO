use egui::{Context, RichText};
use openmmo_common::{DialogueNode, Inventory, MarketOffer, Skill, SkillBook};
use uuid::Uuid;

pub struct GameUi {
    pub show_inventory: bool,
    pub show_bank: bool,
    pub show_skills: bool,
    pub show_chat: bool,
    pub show_quests: bool,
    pub show_market: bool,
    pub show_friends: bool,
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
    pub market_offers: Vec<MarketOffer>,
    pub market_item_id: String,
    pub market_qty: String,
    pub market_price: String,
    pub market_is_buy: bool,
    pub friends: Vec<String>,
    pub online_players: Vec<String>,
    pub friend_add_input: String,
    pub pm_target: String,
    pub trade_partner: Option<String>,
    pub trade_their_items: Vec<openmmo_common::InventorySlot>,
    pub trade_your_items: Vec<openmmo_common::InventorySlot>,
    pub trade_partner_name: String,
}

impl Default for GameUi {
    fn default() -> Self {
        Self {
            show_inventory: true,
            show_bank: false,
            show_skills: true,
            show_chat: true,
            show_quests: true,
            show_market: false,
            show_friends: false,
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
            market_item_id: "2".to_string(),
            market_qty: "1".to_string(),
            market_price: "10".to_string(),
            market_is_buy: false,
            friends: Vec::new(),
            online_players: Vec::new(),
            friend_add_input: String::new(),
            pm_target: String::new(),
            trade_partner: None,
            trade_their_items: Vec::new(),
            trade_your_items: Vec::new(),
            trade_partner_name: String::new(),
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
        inventory: &Inventory,
        bank: &Inventory,
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
                    ui.label(RichText::new(format!("+{amount} {skill} XP")).color(egui::Color32::YELLOW));
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
                        .circle_filled(rect.center(), 4.0, egui::Color32::YELLOW);
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

        egui::SidePanel::right("side_panel").show(ctx, |ui| {
            ui.heading(format!("HP: {hp}/{max_hp}"));
            ui.label(&self.status);
            ui.separator();

            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.show_inventory, true, "Inv");
                ui.selectable_value(&mut self.show_bank, true, "Bank");
                ui.selectable_value(&mut self.show_skills, true, "Skills");
                ui.selectable_value(&mut self.show_quests, true, "Quests");
                ui.selectable_value(&mut self.show_market, true, "Open Market");
                ui.selectable_value(&mut self.show_friends, true, "Friends");
            });

            if self.show_inventory {
                ui.heading("Inventory");
                for (i, slot) in inventory.slots.iter().enumerate() {
                    if let Some(s) = slot {
                        ui.horizontal(|ui| {
                            if ui
                                .button(format!("#{}: item {} x{}", i, s.item_id.0, s.quantity))
                                .clicked()
                            {
                                action = UiAction::DropItem(i);
                            }
                            if ui.small_button("Equip").clicked() {
                                action = UiAction::EquipItem(i);
                            }
                        });
                    }
                }
            }

            if self.show_bank {
                ui.heading("Bank");
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.bank_mode_deposit, true, "Deposit");
                    ui.selectable_value(&mut self.bank_mode_deposit, false, "Withdraw");
                });
                if self.bank_mode_deposit {
                    for (i, slot) in inventory.slots.iter().enumerate() {
                        if let Some(s) = slot {
                            if ui
                                .button(format!("Deposit #{}: item {} x{}", i, s.item_id.0, s.quantity))
                                .clicked()
                            {
                                action = UiAction::BankDeposit { inv_slot: i, quantity: 1 };
                            }
                        }
                    }
                } else {
                    for (i, slot) in bank.slots.iter().enumerate() {
                        if let Some(s) = slot {
                            if ui
                                .button(format!("Withdraw #{}: item {} x{}", i, s.item_id.0, s.quantity))
                                .clicked()
                            {
                                action = UiAction::BankWithdraw {
                                    bank_slot: i,
                                    quantity: 1,
                                };
                            }
                        }
                    }
                }
            }

            if self.show_skills {
                ui.heading("Skills");
                for skill in Skill::all() {
                    let progress = skills.skills.get(skill);
                    let level = progress.map(|p| p.level).unwrap_or(1);
                    let xp = progress.map(|p| p.xp).unwrap_or(0);
                    ui.label(format!("{}: Lv {level} ({xp} xp)", skill.name()));
                }
                ui.separator();
                ui.label("Fabrication");
                for recipe in ["timber_to_plank", "ore_to_ingot", "fish_to_fillet"] {
                    if ui.button(format!("Refine: {recipe}")).clicked() {
                        action = UiAction::Refine {
                            recipe_id: recipe.to_string(),
                        };
                    }
                }
            }

            if self.show_quests {
                ui.heading("Quest Journal");
                for (name, desc) in quest_text {
                    ui.collapsing(name, |ui| {
                        ui.label(desc);
                    });
                }
            }

            if self.show_market {
                ui.heading("Open Market");
                ui.horizontal(|ui| {
                    ui.label("Item id");
                    ui.text_edit_singleline(&mut self.market_item_id);
                });
                ui.horizontal(|ui| {
                    ui.label("Qty");
                    ui.text_edit_singleline(&mut self.market_qty);
                    ui.label("Price");
                    ui.text_edit_singleline(&mut self.market_price);
                });
                ui.checkbox(&mut self.market_is_buy, "Buy offer");
                if ui.button("Place offer").clicked() {
                    if let (Ok(item_id), Ok(qty), Ok(price)) = (
                        self.market_item_id.parse::<u32>(),
                        self.market_qty.parse::<u32>(),
                        self.market_price.parse::<u32>(),
                    ) {
                        action = UiAction::MarketPlaceOffer {
                            item_id: openmmo_common::ItemId(item_id),
                            quantity: qty,
                            price_per: price,
                            is_buy: self.market_is_buy,
                        };
                    }
                }
                for offer in &self.market_offers {
                    ui.label(format!(
                        "{} {} x{} @ {} ({})",
                        if offer.is_buy { "BUY" } else { "SELL" },
                        offer.item_id.0,
                        offer.quantity,
                        offer.price_per,
                        offer.player_name
                    ));
                    if ui.small_button(format!("Cancel {}", offer.id)).clicked() {
                        action = UiAction::MarketCancelOffer { offer_id: offer.id };
                    }
                }
            }

            if self.show_friends {
                ui.heading("Friends");
                ui.horizontal(|ui| {
                    ui.text_edit_singleline(&mut self.friend_add_input);
                    if ui.button("Add").clicked() && !self.friend_add_input.is_empty() {
                        action = UiAction::FriendAdd {
                            name: self.friend_add_input.clone(),
                        };
                        self.friend_add_input.clear();
                    }
                });
                for f in &self.friends {
                    let online = self.online_players.contains(f);
                    ui.label(format!("{f}{}", if online { " (online)" } else { "" }));
                }
                ui.separator();
                ui.label("Private message");
                ui.text_edit_singleline(&mut self.pm_target);
                if ui.button("Send PM").clicked() && !self.pm_target.is_empty() {
                    let parts: Vec<_> = self.pm_target.splitn(2, ':').collect();
                    if parts.len() == 2 {
                        action = UiAction::PrivateMessage {
                            to: parts[0].trim().to_string(),
                            message: parts[1].trim().to_string(),
                        };
                    }
                }
                ui.separator();
                ui.label("Trade");
                ui.text_edit_singleline(&mut self.trade_partner_name);
                if ui.button("Request trade").clicked() && !self.trade_partner_name.is_empty() {
                    action = UiAction::TradeRequestByName {
                        name: self.trade_partner_name.clone(),
                    };
                }
                if let Some(partner) = &self.trade_partner {
                    ui.label(format!("Trading with {partner}"));
                    for item in &self.trade_their_items {
                        ui.label(format!("Their: item {} x{}", item.item_id.0, item.quantity));
                    }
                    for item in &self.trade_your_items {
                        ui.label(format!("Yours: item {} x{}", item.item_id.0, item.quantity));
                    }
                    if ui.button("Accept trade").clicked() {
                        action = UiAction::TradeAccept;
                    }
                }
                if ui.button("Join Arena").clicked() {
                    action = UiAction::JoinMinigame {
                        minigame_id: "arena".to_string(),
                    };
                }
            }
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
                        if ui
                            .button(format!(
                                "Buy item {} x{} for {}",
                                item.item_id.0, item.quantity, item.price
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
    BankDeposit { inv_slot: usize, quantity: u32 },
    BankWithdraw { bank_slot: usize, quantity: u32 },
    Refine { recipe_id: String },
    DialogueSelect {
        npc_entity: openmmo_common::EntityId,
        option_index: usize,
    },
    ShopBuy {
        shop_id: String,
        item_id: openmmo_common::ItemId,
        quantity: u32,
    },
    MarketPlaceOffer {
        item_id: openmmo_common::ItemId,
        quantity: u32,
        price_per: u32,
        is_buy: bool,
    },
    MarketCancelOffer {
        offer_id: Uuid,
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
    TradeAccept,
    JoinMinigame {
        minigame_id: String,
    },
    Connect,
}
