//! The bottom-right side panel: an icon tab strip and one active tab
//! (inventory, equipment, skills, quests, social, settings).

use egui::{Context, RichText, Sense, Ui, Vec2};
use openmmo_common::{
    ContentPack, EquipSlot, Equipment, Inventory, Skill, SkillBook, SkillProgress,
};

use super::grid::{equip_slot, item_grid, GridStyle};
use super::icons::paint_tab_icon;
use super::theme;
use super::{GameUi, UiAction};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelTab {
    Inventory,
    Equipment,
    Skills,
    Quests,
    Social,
    Settings,
}

impl PanelTab {
    pub const ALL: [PanelTab; 6] = [
        PanelTab::Inventory,
        PanelTab::Equipment,
        PanelTab::Skills,
        PanelTab::Quests,
        PanelTab::Social,
        PanelTab::Settings,
    ];

    fn title(self) -> &'static str {
        match self {
            PanelTab::Inventory => "Inventory",
            PanelTab::Equipment => "Worn Equipment",
            PanelTab::Skills => "Skills",
            PanelTab::Quests => "Quest Journal",
            PanelTab::Social => "Friends & Trade",
            PanelTab::Settings => "Settings",
        }
    }
}

pub struct PanelData<'a> {
    pub content: &'a ContentPack,
    pub inventory: &'a Inventory,
    pub equipment: &'a Equipment,
    pub skills: &'a SkillBook,
    pub quest_text: &'a [(String, String)],
    pub combat_opponent: Option<openmmo_common::EntityId>,
}

pub const PANEL_WIDTH: f32 = 214.0;
pub const PANEL_HEIGHT: f32 = 330.0;

pub fn draw_side_panel(ctx: &Context, state: &mut GameUi, data: &PanelData<'_>) -> UiAction {
    let mut action = UiAction::None;
    egui::Area::new(egui::Id::new("side_panel_area"))
        .anchor(egui::Align2::RIGHT_BOTTOM, Vec2::new(-8.0, -8.0))
        .order(egui::Order::Middle)
        .show(ctx, |ui| {
            theme::panel_frame().show(ui, |ui| {
                ui.set_width(PANEL_WIDTH);
                // Tab strip
                ui.horizontal(|ui| {
                    for tab in PanelTab::ALL {
                        let (rect, resp) =
                            ui.allocate_exact_size(Vec2::splat(32.0), Sense::click());
                        let active = state.active_tab == tab;
                        let painter = ui.painter();
                        painter.rect_filled(
                            rect,
                            3.0,
                            if active {
                                theme::SLOT_BG
                            } else if resp.hovered() {
                                egui::Color32::from_rgb(60, 48, 34)
                            } else {
                                theme::PANEL_BG_DARK
                            },
                        );
                        paint_tab_icon(painter, rect, tab, active);
                        let resp = resp.on_hover_text(tab.title());
                        if resp.clicked() {
                            state.active_tab = tab;
                        }
                    }
                });
                ui.separator();
                ui.label(
                    RichText::new(state.active_tab.title())
                        .color(theme::ACCENT)
                        .strong(),
                );
                egui::ScrollArea::vertical()
                    .id_salt("side_panel_scroll")
                    .max_height(PANEL_HEIGHT)
                    .min_scrolled_height(PANEL_HEIGHT)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.set_width(PANEL_WIDTH - 12.0);
                        action = match state.active_tab {
                            PanelTab::Inventory => draw_inventory(ui, state, data),
                            PanelTab::Equipment => draw_equipment(ui, data),
                            PanelTab::Skills => draw_skills(ui, data),
                            PanelTab::Quests => {
                                draw_quests(ui, state, data);
                                UiAction::None
                            }
                            PanelTab::Social => draw_social(ui, state),
                            PanelTab::Settings => draw_settings(ui, state),
                        };
                    });
            });
        });
    action
}

enum InvAction {
    Equip(usize),
    Use(usize),
    Drop(usize),
    Examine(usize),
}

fn draw_inventory(ui: &mut Ui, state: &mut GameUi, data: &PanelData<'_>) -> UiAction {
    let content = data.content;
    let used = data.inventory.slots.iter().filter(|s| s.is_some()).count();
    ui.label(
        RichText::new(format!("{used}/{}", data.inventory.slots.len()))
            .color(theme::TEXT_MUTED)
            .small(),
    );
    let picked = item_grid(
        ui,
        "inventory_grid",
        &GridStyle::inventory(),
        &data.inventory.slots,
        content,
        &|_, _| None,
        &|i, s| {
            let def = content.item(s.item_id)?;
            if def.equip_slot.is_some() {
                Some(InvAction::Equip(i))
            } else if def.heals > 0 {
                Some(InvAction::Use(i))
            } else {
                None
            }
        },
        &mut |ui, i, s| {
            let def = content.item(s.item_id)?;
            let mut out = None;
            if def.heals > 0 && ui.button("Eat").clicked() {
                out = Some(InvAction::Use(i));
            }
            if def.equip_slot.is_some() && ui.button("Wield").clicked() {
                out = Some(InvAction::Equip(i));
            }
            if ui.button("Drop").clicked() {
                out = Some(InvAction::Drop(i));
            }
            if ui.button("Examine").clicked() {
                out = Some(InvAction::Examine(i));
            }
            out
        },
    );
    match picked {
        Some(InvAction::Equip(i)) => UiAction::EquipItem(i),
        Some(InvAction::Use(i)) => UiAction::UseItem(i),
        Some(InvAction::Drop(i)) => UiAction::DropItem(i),
        Some(InvAction::Examine(i)) => {
            if let Some(def) = data.inventory.slots[i]
                .as_ref()
                .and_then(|s| content.item(s.item_id))
            {
                let text = if def.examine.is_empty() {
                    format!("It's a {}.", def.name)
                } else {
                    def.examine.clone()
                };
                state.chat.game(text);
            }
            UiAction::None
        }
        None => UiAction::None,
    }
}

fn draw_equipment(ui: &mut Ui, data: &PanelData<'_>) -> UiAction {
    let content = data.content;
    let eq = data.equipment;
    let mut action = UiAction::None;
    let mut slot_ui = |ui: &mut Ui,
                       label: &str,
                       slot: &Option<openmmo_common::InventorySlot>,
                       kind: EquipSlot| {
        let resp = equip_slot(ui, label, label, slot, content);
        if resp.clicked() && slot.is_some() {
            action = UiAction::UnequipItem(kind);
        }
    };
    ui.vertical_centered(|ui| {
        slot_ui(ui, "Head", &eq.head, EquipSlot::Head);
        ui.horizontal(|ui| {
            ui.add_space(20.0);
            slot_ui(ui, "Weapon", &eq.weapon, EquipSlot::Weapon);
            slot_ui(ui, "Body", &eq.body, EquipSlot::Body);
            slot_ui(ui, "Shield", &eq.shield, EquipSlot::Shield);
        });
        slot_ui(ui, "Legs", &eq.legs, EquipSlot::Legs);
    });
    ui.separator();
    let mut prowess = 0i32;
    let mut fortitude = 0i32;
    let mut electrics = 0i32;
    for slot in [&eq.head, &eq.body, &eq.legs, &eq.weapon, &eq.shield]
        .into_iter()
        .flatten()
    {
        if let Some(def) = content.item(slot.item_id) {
            prowess += def.prowess_bonus;
            fortitude += def.fortitude_bonus;
            electrics += def.electrics_bonus;
        }
    }
    let combat = data.skills.level(Skill::Combat);
    ui.label(format!("Combat level: {combat}"));
    ui.label(format!("Prowess bonus: +{prowess}"));
    ui.label(format!("Fortitude bonus: +{fortitude}"));
    if electrics != 0 {
        ui.label(format!("Electrics bonus: +{electrics}"));
    }
    ui.label(
        RichText::new("Click a slot to remove the item.")
            .color(theme::TEXT_MUTED)
            .small(),
    );
    action
}

fn draw_skills(ui: &mut Ui, data: &PanelData<'_>) -> UiAction {
    let mut action = UiAction::None;
    let skills = data.skills;
    egui::Grid::new("skills_grid")
        .num_columns(2)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            for (i, skill) in Skill::all().iter().enumerate() {
                let progress = skills.skills.get(skill).copied().unwrap_or_default();
                let next = SkillProgress::xp_for_level(progress.level + 1);
                let cur = SkillProgress::xp_for_level(progress.level);
                let frac = if next > cur {
                    ((progress.xp.saturating_sub(cur)) as f64 / (next - cur) as f64) as f32
                } else {
                    1.0
                };
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(skill.name()).size(12.5));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(
                                RichText::new(format!("{}", progress.level))
                                    .color(theme::ACCENT)
                                    .size(12.5),
                            );
                        });
                    });
                    ui.add(
                        egui::ProgressBar::new(frac.clamp(0.0, 1.0))
                            .desired_width(88.0)
                            .desired_height(5.0),
                    )
                    .on_hover_text(format!(
                        "{} xp — {} to level {}",
                        progress.xp,
                        next.saturating_sub(progress.xp),
                        progress.level + 1
                    ));
                });
                if i % 2 == 1 {
                    ui.end_row();
                }
            }
        });
    ui.separator();
    if let Some(target) = data.combat_opponent {
        ui.label(RichText::new("Spells").color(theme::ACCENT).strong());
        let electrics_level = skills.level(Skill::Electrics);
        for spell in &data.content.spells {
            let ok = electrics_level >= spell.electrics_level;
            let label = format!("{} (Electrics {})", spell.name, spell.electrics_level);
            if ui.add_enabled(ok, egui::Button::new(label)).clicked() {
                action = UiAction::CastSpell {
                    target,
                    spell_id: spell.id.clone(),
                };
            }
        }
        ui.separator();
    }
    if !data.content.specializations.is_empty() {
        ui.label(
            RichText::new("Specialization")
                .color(theme::ACCENT)
                .strong(),
        );
        for spec in &data.content.specializations {
            let resp = ui.button(format!("{}: {}", spec.skill.name(), spec.branch));
            let resp = resp.on_hover_text(format!(
                "{} (unlocks at {} {})",
                spec.description,
                spec.skill.name(),
                spec.unlock_level
            ));
            if resp.clicked() {
                action = UiAction::SelectSpecialization {
                    skill: spec.skill,
                    branch: spec.branch.clone(),
                };
            }
        }
    }
    action
}

fn draw_quests(ui: &mut Ui, state: &GameUi, data: &PanelData<'_>) {
    if data.quest_text.is_empty() {
        ui.label(
            RichText::new("No quests yet. Talk to the Wasteland Guide in town.")
                .color(theme::TEXT_MUTED),
        );
    }
    for (name, desc) in data.quest_text {
        ui.collapsing(RichText::new(name).color(theme::ACCENT), |ui| {
            ui.label(desc);
        });
    }
    if let Some(contract) = &state.ledger_contract {
        ui.separator();
        ui.label(
            RichText::new("Ledger contract")
                .color(theme::ACCENT)
                .strong(),
        );
        ui.label(format!("{} — {} left", contract.name, contract.remaining));
        ui.label(
            RichText::new(format!(
                "Reward {} pts · rank {} ({} pts)",
                contract.reward_points, state.ledger_rank, state.ledger_points
            ))
            .color(theme::TEXT_MUTED)
            .small(),
        );
    }
    if !state.collection_log.is_empty() {
        ui.separator();
        ui.collapsing("Collection log", |ui| {
            for (name, _) in &state.collection_log {
                ui.label(name);
            }
        });
    }
}

fn draw_social(ui: &mut Ui, state: &mut GameUi) -> UiAction {
    let mut action = UiAction::None;
    ui.label(RichText::new("Friends").color(theme::ACCENT).strong());
    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(&mut state.friend_add_input)
                .desired_width(120.0)
                .hint_text("name"),
        );
        if ui.button("Add").clicked() && !state.friend_add_input.is_empty() {
            action = UiAction::FriendAdd {
                name: state.friend_add_input.clone(),
            };
            state.friend_add_input.clear();
        }
    });
    if state.friends.is_empty() {
        ui.label(
            RichText::new("No friends added.")
                .color(theme::TEXT_MUTED)
                .small(),
        );
    }
    for f in &state.friends {
        let online = state.online_players.contains(f);
        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(Vec2::splat(8.0), Sense::hover());
            ui.painter().circle_filled(
                rect.center(),
                4.0,
                if online {
                    theme::ONLINE
                } else {
                    theme::OFFLINE
                },
            );
            ui.label(f);
        });
    }
    ui.separator();
    ui.label(RichText::new("Trade").color(theme::ACCENT).strong());
    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(&mut state.trade_partner_name)
                .desired_width(120.0)
                .hint_text("player name"),
        );
        if ui.button("Request").clicked() && !state.trade_partner_name.is_empty() {
            action = UiAction::TradeRequestByName {
                name: state.trade_partner_name.clone(),
            };
        }
    });
    ui.label(
        RichText::new("Or right-click a player in the world.")
            .color(theme::TEXT_MUTED)
            .small(),
    );
    ui.separator();
    ui.label(RichText::new("Activities").color(theme::ACCENT).strong());
    if ui.button("Join Arena").clicked() {
        action = UiAction::JoinMinigame {
            minigame_id: "arena".to_string(),
        };
    }
    action
}

fn draw_settings(ui: &mut Ui, state: &mut GameUi) -> UiAction {
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(Vec2::splat(8.0), Sense::hover());
        ui.painter().circle_filled(
            rect.center(),
            4.0,
            if state.connected {
                theme::ONLINE
            } else {
                theme::OFFLINE
            },
        );
        ui.label(if state.connected {
            "Connected"
        } else {
            "Disconnected"
        });
    });
    ui.label(
        RichText::new(&state.connection_url)
            .color(theme::TEXT_MUTED)
            .small(),
    );
    ui.label(format!("Character: {}", state.character_name));
    ui.separator();
    ui.checkbox(&mut state.show_minimap, "Show minimap");
    ui.checkbox(&mut state.show_hover_text, "Show hover text");
    ui.separator();
    ui.label(RichText::new("Controls").color(theme::ACCENT).strong());
    ui.label(RichText::new("Left click: walk / default action\nRight click: menu\nMiddle drag: rotate camera\nScroll: zoom\nClick the minimap to walk").small());
    if !state.status.is_empty() {
        ui.separator();
        ui.label(
            RichText::new(&state.status)
                .color(theme::TEXT_MUTED)
                .small(),
        );
    }
    UiAction::None
}
