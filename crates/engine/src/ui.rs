use egui::{Context, RichText};
use openmmo_common::{Inventory, Skill, SkillBook};

pub struct GameUi {
    pub show_inventory: bool,
    pub show_bank: bool,
    pub show_skills: bool,
    pub show_chat: bool,
    pub show_quests: bool,
    pub show_ge: bool,
    pub show_friends: bool,
    pub show_minimap: bool,
    pub chat_input: String,
    pub chat_log: Vec<(String, String)>,
    pub connection_url: String,
    pub username: String,
    pub character_name: String,
    pub connected: bool,
    pub status: String,
}

impl Default for GameUi {
    fn default() -> Self {
        Self {
            show_inventory: true,
            show_bank: false,
            show_skills: true,
            show_chat: true,
            show_quests: false,
            show_ge: false,
            show_friends: false,
            show_minimap: true,
            chat_input: String::new(),
            chat_log: Vec::new(),
            connection_url: "ws://127.0.0.1:8080/ws".to_string(),
            username: "player".to_string(),
            character_name: "Adventurer".to_string(),
            connected: false,
            status: "Disconnected".to_string(),
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

        if self.show_minimap {
            egui::Window::new("Minimap")
                .default_pos([10.0, 10.0])
                .resizable(false)
                .show(ctx, |ui| {
                    let (rect, _) = ui.allocate_exact_size(egui::vec2(120.0, 120.0), egui::Sense::hover());
                    ui.painter().rect_filled(rect, 4.0, egui::Color32::from_rgb(20, 40, 20));
                    ui.painter().circle_filled(rect.center(), 4.0, egui::Color32::YELLOW);
                });
        }

        egui::TopBottomPanel::bottom("chat_panel").show(ctx, |ui| {
            egui::ScrollArea::vertical()
                .max_height(80.0)
                .stick_to_bottom(true)
                .show(ui, |ui| {
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
            ui.separator();

            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.show_inventory, true, "Inv");
                ui.selectable_value(&mut self.show_bank, true, "Bank");
                ui.selectable_value(&mut self.show_skills, true, "Skills");
                ui.selectable_value(&mut self.show_quests, true, "Quests");
                ui.selectable_value(&mut self.show_ge, true, "GE");
                ui.selectable_value(&mut self.show_friends, true, "Friends");
            });

            if self.show_inventory {
                ui.heading("Inventory");
                for (i, slot) in inventory.slots.iter().enumerate() {
                    if let Some(s) = slot {
                        if ui.button(format!("#{i}: item {} x{}", s.item_id.0, s.quantity)).clicked() {
                            action = UiAction::DropItem(i);
                        }
                    }
                }
            }

            if self.show_bank {
                ui.heading("Bank");
                for (i, slot) in bank.slots.iter().enumerate() {
                    if let Some(s) = slot {
                        ui.label(format!("#{i}: item {} x{}", s.item_id.0, s.quantity));
                    }
                }
            }

            if self.show_skills {
                ui.heading("Skills");
                for skill in Skill::all() {
                    let level = skills.level(*skill);
                    ui.label(format!("{}: {}", skill.name(), level));
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
        });

        action
    }
}

#[derive(Debug, Clone)]
pub enum UiAction {
    None,
    Chat(String),
    DropItem(usize),
    Connect,
}
