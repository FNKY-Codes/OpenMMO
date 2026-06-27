use egui::{RichText, Ui};
use openmmo_common::{
    ContentPack, Equipment, Inventory, MarketOffer, Skill, SkillBook,
};
use openmmo_protocol::LedgerContract;

use super::theme::ACCENT;
use super::widgets::{
    equipment_strip, item_label, item_label_by_id, item_name, item_row, recipe_tooltip,
    skill_row, ItemRowAction,
};
use super::{GameUi, SidePanelTab, UiAction};

pub fn draw_active_tab(
    ui: &mut Ui,
    game_ui: &mut GameUi,
    content: &ContentPack,
    inventory: &Inventory,
    bank: &Inventory,
    equipment: &Equipment,
    skills: &SkillBook,
    quest_text: &[(String, String)],
) -> UiAction {
    let mut action = UiAction::None;

    let Some(tab) = game_ui.active_tab else {
        return action;
    };

    match tab {
        SidePanelTab::Inventory => {
            action = draw_inventory_tab(ui, game_ui, content, inventory, equipment);
        }
        SidePanelTab::Bank => {
            action = draw_bank_tab(ui, content, inventory, bank, &mut game_ui.bank_mode_deposit);
        }
        SidePanelTab::Skills => {
            action = draw_skills_tab(ui, content, skills);
        }
        SidePanelTab::Quests => {
            draw_quests_tab(ui, quest_text, &game_ui.ledger_contract);
        }
        SidePanelTab::Market => {
            action = draw_market_tab(ui, game_ui, content, inventory);
        }
        SidePanelTab::Friends => {
            action = draw_friends_tab(ui, game_ui);
        }
    }

    action
}

pub fn draw_trade_section(
    ui: &mut Ui,
    game_ui: &mut GameUi,
    content: &ContentPack,
    inventory: &Inventory,
) -> UiAction {
    let mut action = UiAction::None;

    ui.separator();
    ui.heading(RichText::new("Trade").color(ACCENT));

    ui.horizontal(|ui| {
        ui.label("Partner:");
        ui.text_edit_singleline(&mut game_ui.trade_partner_name);
        if ui.button("Request").clicked() && !game_ui.trade_partner_name.is_empty() {
            action = UiAction::TradeRequestByName {
                name: game_ui.trade_partner_name.clone(),
            };
        }
    });

    if let Some(partner) = &game_ui.trade_partner {
        ui.label(format!("Trading with {partner}"));

        ui.label(RichText::new("Their offer:").strong());
        if game_ui.trade_their_items.is_empty() {
            ui.label("  (empty)");
        }
        for item in &game_ui.trade_their_items {
            ui.label(format!("  {}", item_label(content, item)));
        }

        ui.label(RichText::new("Your offer:").strong());
        if game_ui.trade_your_items.is_empty() {
            ui.label("  (empty)");
        }
        for item in &game_ui.trade_your_items {
            ui.label(format!("  {}", item_label(content, item)));
        }

        if !game_ui.trade_pending_items.is_empty() {
            ui.label(RichText::new("Pending:").strong());
            for item in &game_ui.trade_pending_items {
                ui.label(format!("  {}", item_label(content, item)));
            }
            ui.horizontal(|ui| {
                if ui.button("Send offer").clicked() {
                    action = UiAction::TradeOffer {
                        items: game_ui.trade_pending_items.clone(),
                    };
                    game_ui.trade_pending_items.clear();
                }
                if ui.button("Clear pending").clicked() {
                    game_ui.trade_pending_items.clear();
                }
            });
        }

        ui.label("Add from inventory:");
        for (_i, slot) in inventory.slots.iter().enumerate() {
            if let Some(s) = slot {
                ui.horizontal(|ui| {
                    ui.label(item_label(content, s));
                    if ui.small_button("Add").clicked() {
                        game_ui.trade_pending_items.push(s.clone());
                    }
                });
            }
        }

        if ui.button("Accept trade").clicked() {
            action = UiAction::TradeAccept;
        }
    }

    action
}

pub fn draw_activities_footer(ui: &mut Ui) -> UiAction {
    let mut action = UiAction::None;
    ui.separator();
    if ui.button("Join Arena").clicked() {
        action = UiAction::JoinMinigame {
            minigame_id: "arena".to_string(),
        };
    }
    action
}

fn draw_inventory_tab(
    ui: &mut Ui,
    _game_ui: &mut GameUi,
    content: &ContentPack,
    inventory: &Inventory,
    equipment: &Equipment,
) -> UiAction {
    let mut action = UiAction::None;
    ui.heading("Inventory");

    if let Some(slot) = equipment_strip(ui, content, equipment) {
        return UiAction::UnequipItem(slot);
    }
    ui.separator();

    let mut empty = true;
    for (i, slot) in inventory.slots.iter().enumerate() {
        if let Some(s) = slot {
            empty = false;
            match item_row(ui, content, s, i, true) {
                ItemRowAction::Drop(idx) => action = UiAction::DropItem(idx),
                ItemRowAction::Equip(idx) => action = UiAction::EquipItem(idx),
                ItemRowAction::None => {}
            }
        }
    }
    if empty {
        ui.label("No items.");
    }
    action
}

fn draw_bank_tab(
    ui: &mut Ui,
    content: &ContentPack,
    inventory: &Inventory,
    bank: &Inventory,
    bank_mode_deposit: &mut bool,
) -> UiAction {
    let mut action = UiAction::None;
    ui.heading("Bank");
    ui.horizontal(|ui| {
        ui.selectable_value(bank_mode_deposit, true, "Deposit");
        ui.selectable_value(bank_mode_deposit, false, "Withdraw");
    });

    if *bank_mode_deposit {
        let mut empty = true;
        for (i, slot) in inventory.slots.iter().enumerate() {
            if let Some(s) = slot {
                empty = false;
                if ui
                    .button(format!("Deposit #{}: {}", i, item_label(content, s)))
                    .clicked()
                {
                    action = UiAction::BankDeposit { inv_slot: i, quantity: 1 };
                }
            }
        }
        if empty {
            ui.label("No items to deposit.");
        }
    } else {
        let mut empty = true;
        for (i, slot) in bank.slots.iter().enumerate() {
            if let Some(s) = slot {
                empty = false;
                if ui
                    .button(format!("Withdraw #{}: {}", i, item_label(content, s)))
                    .clicked()
                {
                    action = UiAction::BankWithdraw {
                        bank_slot: i,
                        quantity: 1,
                    };
                }
            }
        }
        if empty {
            ui.label("Bank is empty.");
        }
    }
    action
}

fn draw_skills_tab(ui: &mut Ui, content: &ContentPack, skills: &SkillBook) -> UiAction {
    let mut action = UiAction::None;
    ui.heading("Skills");
    for skill in Skill::all() {
        let progress = skills.skills.get(skill);
        let level = progress.map(|p| p.level).unwrap_or(1);
        let xp = progress.map(|p| p.xp).unwrap_or(0);
        skill_row(ui, *skill, level, xp);
    }

    ui.separator();
    ui.label(RichText::new("Fabrication").strong().color(ACCENT));
    let fab_level = skills
        .skills
        .get(&Skill::Fabrication)
        .map(|p| p.level)
        .unwrap_or(1);

    let mut any_recipe = false;
    for recipe in &content.recipes {
        if fab_level >= recipe.fabrication_level {
            any_recipe = true;
            let response = ui.button(&recipe.name);
            let response =
                response.on_hover_ui(|ui| recipe_tooltip(ui, content, recipe));
            if response.clicked() {
                action = UiAction::Refine {
                    recipe_id: recipe.id.clone(),
                };
            }
        }
    }
    if !any_recipe {
        ui.label("No recipes available at your level.");
    }
    action
}

fn draw_quests_tab(
    ui: &mut Ui,
    quest_text: &[(String, String)],
    ledger_contract: &Option<LedgerContract>,
) {
    ui.heading("Quest Journal");
    if quest_text.is_empty() {
        ui.label("No active quests.");
    }
    for (name, desc) in quest_text {
        ui.collapsing(name, |ui| {
            ui.label(desc);
        });
    }

    if let Some(contract) = ledger_contract {
        ui.separator();
        ui.collapsing("Ledger Contract", |ui| {
            ui.label(format!("Target: {}", contract.name));
            ui.label(format!("Remaining: {}", contract.remaining));
            ui.label(format!("Reward: {} pts", contract.reward_points));
        });
    }
}

fn draw_market_tab(
    ui: &mut Ui,
    game_ui: &mut GameUi,
    content: &ContentPack,
    inventory: &Inventory,
) -> UiAction {
    let mut action = UiAction::None;
    ui.heading("Open Market");

    ui.label("Item");
    egui::ComboBox::from_id_salt("market_item_picker")
        .selected_text(market_item_display(game_ui, content))
        .show_ui(ui, |ui| {
            for (idx, slot) in inventory.slots.iter().enumerate() {
                if let Some(s) = slot {
                    let label = format!("#{idx}: {}", item_label(content, s));
                    let selected = game_ui.market_selected_inv_slot == Some(idx)
                        && game_ui.market_item_id == s.item_id;
                    if ui.selectable_label(selected, label).clicked() {
                        game_ui.market_selected_inv_slot = Some(idx);
                        game_ui.market_item_id = s.item_id;
                    }
                }
            }
            ui.separator();
            for item in &content.items {
                let selected = game_ui.market_selected_inv_slot.is_none()
                    && game_ui.market_item_id == item.id;
                if ui
                    .selectable_label(selected, format!("{} (id {})", item.name, item.id.0))
                    .clicked()
                {
                    game_ui.market_item_id = item.id;
                    game_ui.market_selected_inv_slot = None;
                }
            }
        });

    ui.horizontal(|ui| {
        ui.label("Qty");
        ui.text_edit_singleline(&mut game_ui.market_qty);
        ui.label("Price");
        ui.text_edit_singleline(&mut game_ui.market_price);
    });
    ui.checkbox(&mut game_ui.market_is_buy, "Buy offer");

    let qty = game_ui.market_qty.parse::<u32>();
    let price = game_ui.market_price.parse::<u32>();
    if game_ui.market_qty.parse::<u32>().is_err() && !game_ui.market_qty.is_empty() {
        ui.colored_label(egui::Color32::LIGHT_RED, "Invalid quantity");
    }
    if game_ui.market_price.parse::<u32>().is_err() && !game_ui.market_price.is_empty() {
        ui.colored_label(egui::Color32::LIGHT_RED, "Invalid price");
    }

    if ui.button("Place offer").clicked() {
        match (qty, price) {
            (Ok(qty), Ok(price)) if qty > 0 && price > 0 => {
                action = UiAction::MarketPlaceOffer {
                    item_id: game_ui.market_item_id,
                    quantity: qty,
                    price_per: price,
                    is_buy: game_ui.market_is_buy,
                };
            }
            _ => {
                game_ui.status = "Market: enter valid quantity and price".into();
            }
        }
    }

    ui.separator();
    ui.label(RichText::new("Active offers").strong());
    if game_ui.market_offers.is_empty() {
        ui.label("No offers.");
    }
    for offer in &game_ui.market_offers.clone() {
        draw_market_offer_row(ui, content, offer, &mut action);
    }
    action
}

fn market_item_display(game_ui: &GameUi, content: &ContentPack) -> String {
    if let Some(idx) = game_ui.market_selected_inv_slot {
        format!("#{idx}: {}", item_name(content, game_ui.market_item_id))
    } else {
        item_name(content, game_ui.market_item_id).to_string()
    }
}

fn draw_market_offer_row(
    ui: &mut Ui,
    content: &ContentPack,
    offer: &MarketOffer,
    action: &mut UiAction,
) {
    ui.horizontal(|ui| {
        ui.label(format!(
            "{} {} @ {} ({})",
            if offer.is_buy { "BUY" } else { "SELL" },
            item_label_by_id(content, offer.item_id, offer.quantity),
            offer.price_per,
            offer.player_name
        ));
        if ui.small_button("Cancel").clicked() {
            *action = UiAction::MarketCancelOffer { offer_id: offer.id };
        }
    });
}

fn draw_friends_tab(ui: &mut Ui, game_ui: &mut GameUi) -> UiAction {
    let mut action = UiAction::None;
    ui.heading("Friends");

    ui.horizontal(|ui| {
        ui.text_edit_singleline(&mut game_ui.friend_add_input);
        if ui.button("Add").clicked() && !game_ui.friend_add_input.is_empty() {
            action = UiAction::FriendAdd {
                name: game_ui.friend_add_input.clone(),
            };
            game_ui.friend_add_input.clear();
        }
    });

    if game_ui.friends.is_empty() {
        ui.label("No friends yet.");
    }
    for f in &game_ui.friends {
        let online = game_ui.online_players.contains(f);
        let status = if online { " (online)" } else { "" };
        ui.label(format!("{f}{status}"));
    }

    ui.separator();
    ui.collapsing("Private Message", |ui| {
        ui.label("To");
        ui.text_edit_singleline(&mut game_ui.pm_to);
        ui.label("Message");
        ui.text_edit_multiline(&mut game_ui.pm_message);
        if ui.button("Send PM").clicked()
            && !game_ui.pm_to.is_empty()
            && !game_ui.pm_message.is_empty()
        {
            action = UiAction::PrivateMessage {
                to: game_ui.pm_to.clone(),
                message: game_ui.pm_message.clone(),
            };
            game_ui.pm_message.clear();
        }
    });

    action
}
