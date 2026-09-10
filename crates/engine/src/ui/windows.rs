//! Interface windows opened by the world: bank, shop, crafting station,
//! trade board, NPC dialogue and player trade.

use egui::{Context, RichText, Ui, Vec2};
use openmmo_common::{ContentPack, Inventory, InventorySlot, SkillBook, StationTag};

use super::grid::{item_grid, GridStyle};
use super::icons::paint_item_icon;
use super::theme;
use super::widgets::item_name;
use super::{GameUi, UiAction};

fn window<'a>(title: &'a str, id: &'static str) -> egui::Window<'a> {
    egui::Window::new(RichText::new(title).color(theme::ACCENT).strong())
        .id(egui::Id::new(id))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, Vec2::new(-60.0, -40.0))
}

enum Qty {
    N(u32),
    All,
}

fn qty_menu(ui: &mut Ui, verb: &str, slot: &InventorySlot) -> Option<Qty> {
    let mut out = None;
    for n in [1u32, 5, 10] {
        if n <= slot.quantity && ui.button(format!("{verb} {n}")).clicked() {
            out = Some(Qty::N(n));
        }
    }
    if slot.quantity > 1 && ui.button(format!("{verb} all")).clicked() {
        out = Some(Qty::All);
    }
    if slot.quantity == 1 && out.is_none() && ui.button(format!("{verb} 1")).clicked() {
        out = Some(Qty::N(1));
    }
    out
}

fn resolve(q: Qty, slot: &InventorySlot) -> u32 {
    match q {
        Qty::N(n) => n.min(slot.quantity),
        Qty::All => slot.quantity,
    }
}

pub fn draw_bank(
    ctx: &Context,
    state: &mut GameUi,
    content: &ContentPack,
    inventory: &Inventory,
    bank: &Inventory,
) -> UiAction {
    let mut action = UiAction::None;
    let mut open = true;
    let used = bank.slots.iter().filter(|s| s.is_some()).count();
    window(
        &format!("Bank of the Reach — {used}/{} ", bank.slots.len()),
        "bank_window",
    )
    .open(&mut open)
    .show(ctx, |ui| {
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.label(RichText::new("Bank").color(theme::TEXT_MUTED).small());
                egui::ScrollArea::vertical()
                    .id_salt("bank_scroll")
                    .max_height(300.0)
                    .show(ui, |ui| {
                        // Only show rows that could hold something: filled slots
                        // plus one empty row so the bank doesn't look infinite.
                        let last = bank
                            .slots
                            .iter()
                            .rposition(|s| s.is_some())
                            .map(|i| i + 1)
                            .unwrap_or(0);
                        let shown = ((last + 8) / 8 * 8).min(bank.slots.len()).max(8);
                        let picked = item_grid(
                            ui,
                            "bank_grid",
                            &GridStyle::wide(8),
                            &bank.slots[..shown],
                            content,
                            &|_, _| None,
                            &|i, _| Some((i, Qty::N(1))),
                            &mut |ui, i, s| qty_menu(ui, "Withdraw", s).map(|q| (i, q)),
                        );
                        if let Some((i, q)) = picked {
                            if let Some(s) = &bank.slots[i] {
                                action = UiAction::BankWithdraw {
                                    bank_slot: i,
                                    quantity: resolve(q, s),
                                };
                            }
                        }
                    });
            });
            ui.separator();
            ui.vertical(|ui| {
                ui.label(RichText::new("Inventory").color(theme::TEXT_MUTED).small());
                let picked = item_grid(
                    ui,
                    "bank_inventory_grid",
                    &GridStyle::inventory(),
                    &inventory.slots,
                    content,
                    &|_, _| None,
                    &|i, _| Some((i, Qty::N(1))),
                    &mut |ui, i, s| qty_menu(ui, "Deposit", s).map(|q| (i, q)),
                );
                if let Some((i, q)) = picked {
                    if let Some(s) = &inventory.slots[i] {
                        action = UiAction::BankDeposit {
                            inv_slot: i,
                            quantity: resolve(q, s),
                        };
                    }
                }
            });
        });
        ui.label(
            RichText::new("Click to move one · right-click for more")
                .color(theme::TEXT_MUTED)
                .small(),
        );
    });
    if !open {
        state.open_station = None;
    }
    action
}

pub fn draw_shop(
    ctx: &Context,
    state: &mut GameUi,
    content: &ContentPack,
    inventory: &Inventory,
) -> UiAction {
    let mut action = UiAction::None;
    let Some((shop_id, stock)) = state.open_shop.clone() else {
        return action;
    };
    let title = content
        .shops
        .iter()
        .find(|s| s.id == shop_id)
        .map(|s| s.name.clone())
        .unwrap_or_else(|| shop_id.clone());
    let scrap: u32 = inventory
        .slots
        .iter()
        .flatten()
        .filter(|s| s.item_id.0 == 1)
        .map(|s| s.quantity)
        .sum();
    let mut open = true;
    window(&title, "shop_window")
        .open(&mut open)
        .show(ctx, |ui| {
            ui.label(RichText::new(format!("You have {scrap} Scrap")).color(theme::ACCENT));
            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    ui.label(RichText::new("For sale").color(theme::TEXT_MUTED).small());
                    let slots: Vec<Option<InventorySlot>> = stock
                        .iter()
                        .map(|s| {
                            Some(InventorySlot {
                                item_id: s.item_id,
                                quantity: s.quantity,
                            })
                        })
                        .collect();
                    let picked = item_grid(
                        ui,
                        "shop_stock_grid",
                        &GridStyle::shop(5),
                        &slots,
                        content,
                        &|i, _| Some(format!("{}", stock[i].price)),
                        &|i, _| Some((i, 1u32)),
                        &mut |ui, i, _| {
                            let mut out = None;
                            for n in [1u32, 5, 10] {
                                if ui.button(format!("Buy {n}")).clicked() {
                                    out = Some((i, n));
                                }
                            }
                            out
                        },
                    );
                    if let Some((i, n)) = picked {
                        action = UiAction::ShopBuy {
                            shop_id: shop_id.clone(),
                            item_id: stock[i].item_id,
                            quantity: n,
                        };
                    }
                });
                ui.separator();
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new("Sell from inventory")
                            .color(theme::TEXT_MUTED)
                            .small(),
                    );
                    let picked = item_grid(
                        ui,
                        "shop_sell_grid",
                        &GridStyle::inventory(),
                        &inventory.slots,
                        content,
                        &|_, _| None,
                        &|i, s| (s.item_id.0 != 1).then_some((i, Qty::N(1))),
                        &mut |ui, i, s| {
                            if s.item_id.0 == 1 {
                                ui.label("Already Scrap");
                                return None;
                            }
                            let value = content
                                .item(s.item_id)
                                .map(|d| d.alchemy_value.max(1))
                                .unwrap_or(1);
                            ui.label(RichText::new(format!("{value} Scrap each")).small());
                            qty_menu(ui, "Sell", s).map(|q| (i, q))
                        },
                    );
                    if let Some((i, q)) = picked {
                        if let Some(s) = &inventory.slots[i] {
                            action = UiAction::ShopSell {
                                shop_id: shop_id.clone(),
                                inv_slot: i,
                                quantity: resolve(q, s),
                            };
                        }
                    }
                });
            });
            ui.label(
                RichText::new("Click to buy/sell one · right-click for more")
                    .color(theme::TEXT_MUTED)
                    .small(),
            );
        });
    if !open {
        state.open_shop = None;
    }
    action
}

pub fn draw_crafting(
    ctx: &Context,
    state: &mut GameUi,
    content: &ContentPack,
    inventory: &Inventory,
    skills: &SkillBook,
) -> UiAction {
    let mut action = UiAction::None;
    let Some((station, _)) = state.open_station else {
        return action;
    };
    let title = match station {
        StationTag::Furnace => "Furnace",
        StationTag::Anvil => "Anvil",
        StationTag::Workbench => "Workbench",
        StationTag::Cooking => "Cookfire",
        StationTag::Bank | StationTag::Market => return action,
    };
    let level = skills.level(openmmo_common::Skill::Fabrication);
    let have = |item: openmmo_common::ItemId| -> u32 {
        inventory
            .slots
            .iter()
            .flatten()
            .filter(|s| s.item_id == item)
            .map(|s| s.quantity)
            .sum()
    };
    let mut open = true;
    window(title, "craft_window")
        .open(&mut open)
        .show(ctx, |ui| {
            ui.label(
                RichText::new(format!("Fabrication level {level}"))
                    .color(theme::TEXT_MUTED)
                    .small(),
            );
            if let Some((id, n)) = &state.craft_queue {
                ui.label(
                    RichText::new(format!("Making {} ({} left)…", id, n)).color(theme::ACCENT),
                );
            }
            egui::ScrollArea::vertical()
                .id_salt("craft_scroll")
                .max_height(320.0)
                .show(ui, |ui| {
                    let mut any = false;
                    for recipe in content
                        .recipes
                        .iter()
                        .filter(|r| r.station == Some(station))
                    {
                        any = true;
                        let can_level = level >= recipe.fabrication_level;
                        let can_inputs =
                            recipe.inputs.iter().all(|i| have(i.item_id) >= i.quantity);
                        ui.horizontal(|ui| {
                            let (rect, _) =
                                ui.allocate_exact_size(Vec2::splat(34.0), egui::Sense::hover());
                            ui.painter().rect_filled(rect, 3.0, theme::SLOT_BG);
                            if let Some(def) = content.item(recipe.output) {
                                paint_item_icon(ui.painter(), rect, def);
                            }
                            ui.vertical(|ui| {
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new(&recipe.name).strong());
                                    ui.label(
                                        RichText::new(format!("lvl {}", recipe.fabrication_level))
                                            .color(if can_level {
                                                theme::TEXT_MUTED
                                            } else {
                                                theme::DANGER
                                            })
                                            .small(),
                                    );
                                });
                                let inputs: Vec<String> = recipe
                                    .inputs
                                    .iter()
                                    .map(|i| {
                                        format!("{} x{}", item_name(content, i.item_id), i.quantity)
                                    })
                                    .collect();
                                ui.label(
                                    RichText::new(inputs.join(", "))
                                        .color(if can_inputs {
                                            theme::TEXT
                                        } else {
                                            theme::DANGER
                                        })
                                        .small(),
                                );
                            });
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    for n in [10u32, 5, 1] {
                                        if ui
                                            .add_enabled(
                                                can_level && can_inputs,
                                                egui::Button::new(format!("{n}")),
                                            )
                                            .clicked()
                                        {
                                            action = UiAction::Craft {
                                                recipe_id: recipe.id.clone(),
                                                count: n,
                                            };
                                        }
                                    }
                                    ui.label(RichText::new("Make").small());
                                },
                            );
                        });
                        ui.separator();
                    }
                    if !any {
                        ui.label("Nothing can be made here.");
                    }
                });
        });
    if !open {
        state.open_station = None;
    }
    action
}

pub fn draw_market(
    ctx: &Context,
    state: &mut GameUi,
    content: &ContentPack,
    inventory: &Inventory,
) -> UiAction {
    let mut action = UiAction::None;
    if !matches!(state.open_station, Some((StationTag::Market, _))) {
        return action;
    }
    let mut open = true;
    window("Trade Board", "market_window")
        .open(&mut open)
        .show(ctx, |ui| {
            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new("Post an offer")
                            .color(theme::TEXT_MUTED)
                            .small(),
                    );
                    ui.horizontal(|ui| {
                        ui.selectable_value(&mut state.market_is_buy, false, "Sell");
                        ui.selectable_value(&mut state.market_is_buy, true, "Buy");
                    });
                    if state.market_is_buy {
                        egui::ComboBox::from_id_salt("market_buy_item")
                            .width(160.0)
                            .selected_text(item_name(content, state.market_item_id))
                            .show_ui(ui, |ui| {
                                for item in &content.items {
                                    ui.selectable_value(
                                        &mut state.market_item_id,
                                        item.id,
                                        &item.name,
                                    );
                                }
                            });
                    } else {
                        ui.label(RichText::new("Pick an item from your inventory:").small());
                        let picked = item_grid(
                            ui,
                            "market_inventory_grid",
                            &GridStyle::inventory(),
                            &inventory.slots,
                            content,
                            &|_, _| None,
                            &|i, s| Some((i, s.item_id)),
                            &mut |_, _, _| None,
                        );
                        if let Some((i, item)) = picked {
                            state.market_selected_inv_slot = Some(i);
                            state.market_item_id = item;
                        }
                        ui.label(format!(
                            "Selected: {}",
                            item_name(content, state.market_item_id)
                        ));
                    }
                    ui.horizontal(|ui| {
                        ui.label("Qty");
                        ui.add(
                            egui::TextEdit::singleline(&mut state.market_qty).desired_width(50.0),
                        );
                        ui.label("Each");
                        ui.add(
                            egui::TextEdit::singleline(&mut state.market_price).desired_width(60.0),
                        );
                    });
                    if ui.button("Post offer").clicked() {
                        match (
                            state.market_qty.parse::<u32>(),
                            state.market_price.parse::<u32>(),
                        ) {
                            (Ok(q), Ok(p)) if q > 0 && p > 0 => {
                                action = UiAction::MarketPlaceOffer {
                                    item_id: state.market_item_id,
                                    quantity: q,
                                    price_per: p,
                                    is_buy: state.market_is_buy,
                                };
                            }
                            _ => state.chat.error("Enter a valid quantity and price."),
                        }
                    }
                });
                ui.separator();
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new("Open offers")
                            .color(theme::TEXT_MUTED)
                            .small(),
                    );
                    egui::ScrollArea::vertical()
                        .id_salt("market_offers")
                        .max_height(260.0)
                        .show(ui, |ui| {
                            ui.set_min_width(260.0);
                            if state.market_offers.is_empty() {
                                ui.label("No offers posted.");
                            }
                            for offer in state.market_offers.clone() {
                                ui.horizontal(|ui| {
                                    let kind = if offer.is_buy { "BUY" } else { "SELL" };
                                    ui.label(
                                        RichText::new(kind)
                                            .color(if offer.is_buy {
                                                theme::GOOD
                                            } else {
                                                theme::ACCENT
                                            })
                                            .small(),
                                    );
                                    ui.label(format!(
                                        "{} x{} @ {}",
                                        item_name(content, offer.item_id),
                                        offer.quantity,
                                        offer.price_per
                                    ));
                                    ui.label(
                                        RichText::new(&offer.player_name)
                                            .color(theme::TEXT_MUTED)
                                            .small(),
                                    );
                                    if offer.player_name == state.character_name
                                        && ui.small_button("Cancel").clicked()
                                    {
                                        action = UiAction::MarketCancelOffer { offer_id: offer.id };
                                    }
                                });
                            }
                        });
                });
            });
        });
    if !open {
        state.open_station = None;
    }
    action
}

/// NPC dialogue in a chat-box-style panel above the chat.
pub fn draw_dialogue(ctx: &Context, state: &mut GameUi, content: &ContentPack) -> UiAction {
    let mut action = UiAction::None;
    let Some((npc_entity, node)) = state.active_dialogue.clone() else {
        return action;
    };
    let speaker = node
        .id
        .strip_prefix("npc_")
        .and_then(|rest| rest.split('_').next())
        .and_then(|id| id.parse::<u32>().ok())
        .and_then(|id| content.npc(openmmo_common::NpcId(id)))
        .map(|d| d.name.clone())
        .unwrap_or_else(|| "…".into());
    let pressed_number = (1..=9u8).find(|n| {
        let key = match n {
            1 => egui::Key::Num1,
            2 => egui::Key::Num2,
            3 => egui::Key::Num3,
            4 => egui::Key::Num4,
            5 => egui::Key::Num5,
            6 => egui::Key::Num6,
            7 => egui::Key::Num7,
            8 => egui::Key::Num8,
            _ => egui::Key::Num9,
        };
        ctx.input(|i| i.key_pressed(key))
    });
    egui::Area::new(egui::Id::new("dialogue_area"))
        .anchor(egui::Align2::LEFT_BOTTOM, Vec2::new(8.0, -196.0))
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            theme::panel_frame().show(ui, |ui| {
                ui.set_width(520.0);
                ui.label(RichText::new(&speaker).color(theme::ACCENT).strong());
                ui.label(&node.text);
                ui.separator();
                for (idx, opt) in node.options.iter().enumerate() {
                    let label = format!("{}. {}", idx + 1, opt.label);
                    let clicked = ui
                        .add(
                            egui::Button::new(RichText::new(label).color(theme::HOVER_TEXT))
                                .frame(false),
                        )
                        .clicked();
                    if clicked || pressed_number == Some((idx + 1) as u8) {
                        action = UiAction::DialogueSelect {
                            npc_entity,
                            dialogue_id: node.id.clone(),
                            option_index: idx,
                        };
                        state.active_dialogue = None;
                    }
                }
                if node.options.is_empty() && ui.button("Continue").clicked() {
                    state.active_dialogue = None;
                }
            });
        });
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        state.active_dialogue = None;
    }
    action
}

pub fn draw_trade(
    ctx: &Context,
    state: &mut GameUi,
    content: &ContentPack,
    inventory: &Inventory,
) -> UiAction {
    let mut action = UiAction::None;
    let Some(partner) = state.trade_partner.clone() else {
        return action;
    };
    let mut open = true;
    window(&format!("Trading with {partner}"), "trade_window")
        .open(&mut open)
        .show(ctx, |ui| {
            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    ui.label(RichText::new("Your offer").color(theme::TEXT_MUTED).small());
                    let mut yours: Vec<Option<InventorySlot>> =
                        state.trade_your_items.iter().cloned().map(Some).collect();
                    yours.extend(state.trade_pending_items.iter().cloned().map(Some));
                    while yours.len() < 8 {
                        yours.push(None);
                    }
                    item_grid::<()>(
                        ui,
                        "trade_yours",
                        &GridStyle::wide(4),
                        &yours,
                        content,
                        &|_, _| None,
                        &|_, _| None,
                        &mut |_, _, _| None,
                    );
                    ui.label(
                        RichText::new("Their offer")
                            .color(theme::TEXT_MUTED)
                            .small(),
                    );
                    let mut theirs: Vec<Option<InventorySlot>> =
                        state.trade_their_items.iter().cloned().map(Some).collect();
                    while theirs.len() < 8 {
                        theirs.push(None);
                    }
                    item_grid::<()>(
                        ui,
                        "trade_theirs",
                        &GridStyle::wide(4),
                        &theirs,
                        content,
                        &|_, _| None,
                        &|_, _| None,
                        &mut |_, _, _| None,
                    );
                    ui.horizontal(|ui| {
                        if !state.trade_pending_items.is_empty()
                            && ui.button("Send offer").clicked()
                        {
                            action = UiAction::TradeOffer {
                                items: state.trade_pending_items.clone(),
                            };
                            state.trade_pending_items.clear();
                        }
                        if ui.button("Accept").clicked() {
                            action = UiAction::TradeAccept;
                        }
                    });
                });
                ui.separator();
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new("Add from inventory")
                            .color(theme::TEXT_MUTED)
                            .small(),
                    );
                    let picked = item_grid(
                        ui,
                        "trade_inventory",
                        &GridStyle::inventory(),
                        &inventory.slots,
                        content,
                        &|_, _| None,
                        &|i, _| Some((i, Qty::N(1))),
                        &mut |ui, i, s| qty_menu(ui, "Offer", s).map(|q| (i, q)),
                    );
                    if let Some((i, q)) = picked {
                        if let Some(s) = &inventory.slots[i] {
                            state.trade_pending_items.push(InventorySlot {
                                item_id: s.item_id,
                                quantity: resolve(q, s),
                            });
                        }
                    }
                });
            });
        });
    if !open {
        state.trade_partner = None;
        state.trade_pending_items.clear();
    }
    action
}
