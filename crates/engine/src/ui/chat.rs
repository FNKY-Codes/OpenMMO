//! Chat box: game messages, player chat and combat text in one scrollback
//! with filter tabs, bottom-left of the screen.

use egui::{Color32, Context, RichText, Vec2};
use openmmo_protocol::ChatChannel;

use super::theme;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatKind {
    Game,
    Error,
    Combat,
    Local,
    Global,
    Clan,
    Private,
}

#[derive(Debug, Clone)]
pub struct ChatLine {
    pub kind: ChatKind,
    pub from: String,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatTab {
    All,
    Game,
    Chat,
    Clan,
    Private,
}

impl ChatTab {
    fn shows(self, kind: ChatKind) -> bool {
        match self {
            ChatTab::All => true,
            ChatTab::Game => matches!(kind, ChatKind::Game | ChatKind::Error | ChatKind::Combat),
            ChatTab::Chat => matches!(kind, ChatKind::Local | ChatKind::Global),
            ChatTab::Clan => kind == ChatKind::Clan,
            ChatTab::Private => kind == ChatKind::Private,
        }
    }
}

pub const MAX_LINES: usize = 250;

fn color(kind: ChatKind) -> Color32 {
    match kind {
        ChatKind::Game => theme::CHAT_GAME,
        ChatKind::Error => theme::CHAT_ERROR,
        ChatKind::Combat => theme::CHAT_COMBAT,
        ChatKind::Local => theme::CHAT_LOCAL,
        ChatKind::Global => theme::CHAT_GLOBAL,
        ChatKind::Clan => theme::CHAT_CLAN,
        ChatKind::Private => theme::CHAT_PRIVATE,
    }
}

pub struct ChatState {
    pub lines: Vec<ChatLine>,
    pub tab: ChatTab,
    pub input: String,
    pub send_channel: ChatChannel,
    pub pm_target: String,
}

impl Default for ChatState {
    fn default() -> Self {
        Self {
            lines: Vec::new(),
            tab: ChatTab::All,
            input: String::new(),
            send_channel: ChatChannel::Local,
            pm_target: String::new(),
        }
    }
}

impl ChatState {
    pub fn push(&mut self, kind: ChatKind, from: impl Into<String>, text: impl Into<String>) {
        self.lines.push(ChatLine {
            kind,
            from: from.into(),
            text: text.into(),
        });
        if self.lines.len() > MAX_LINES {
            let drop = self.lines.len() - MAX_LINES;
            self.lines.drain(0..drop);
        }
    }

    pub fn game(&mut self, text: impl Into<String>) {
        self.push(ChatKind::Game, "", text);
    }

    pub fn error(&mut self, text: impl Into<String>) {
        self.push(ChatKind::Error, "", text);
    }
}

pub enum ChatOutput {
    Say(ChatChannel, String),
    Whisper(String, String),
}

/// Draw the chat box. Returns text the player submitted.
pub fn draw_chat(ctx: &Context, chat: &mut ChatState) -> Option<ChatOutput> {
    let mut out = None;
    egui::Area::new(egui::Id::new("chat_area"))
        .anchor(egui::Align2::LEFT_BOTTOM, Vec2::new(8.0, -8.0))
        .order(egui::Order::Middle)
        .show(ctx, |ui| {
            theme::panel_frame().show(ui, |ui| {
                ui.set_width(520.0);
                ui.horizontal(|ui| {
                    for (tab, label) in [
                        (ChatTab::All, "All"),
                        (ChatTab::Game, "Game"),
                        (ChatTab::Chat, "Chat"),
                        (ChatTab::Clan, "Clan"),
                        (ChatTab::Private, "Private"),
                    ] {
                        if ui.selectable_label(chat.tab == tab, label).clicked() {
                            chat.tab = tab;
                        }
                    }
                });
                egui::ScrollArea::vertical()
                    .id_salt("chat_scroll")
                    .max_height(118.0)
                    .min_scrolled_height(118.0)
                    .auto_shrink([false, false])
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        ui.set_width(508.0);
                        for line in chat.lines.iter().filter(|l| chat.tab.shows(l.kind)) {
                            let text = if line.from.is_empty() {
                                line.text.clone()
                            } else {
                                match line.kind {
                                    ChatKind::Private => {
                                        format!("From {}: {}", line.from, line.text)
                                    }
                                    ChatKind::Clan => {
                                        format!("[Clan] {}: {}", line.from, line.text)
                                    }
                                    ChatKind::Global => {
                                        format!("[World] {}: {}", line.from, line.text)
                                    }
                                    _ => format!("{}: {}", line.from, line.text),
                                }
                            };
                            ui.label(RichText::new(text).color(color(line.kind)).size(13.0));
                        }
                    });
                ui.horizontal(|ui| {
                    egui::ComboBox::from_id_salt("chat_send_channel")
                        .width(70.0)
                        .selected_text(match chat.send_channel {
                            ChatChannel::Local => "Local",
                            ChatChannel::Global => "World",
                            ChatChannel::Clan => "Clan",
                            ChatChannel::Private => "PM",
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut chat.send_channel,
                                ChatChannel::Local,
                                "Local",
                            );
                            ui.selectable_value(
                                &mut chat.send_channel,
                                ChatChannel::Global,
                                "World",
                            );
                            ui.selectable_value(&mut chat.send_channel, ChatChannel::Clan, "Clan");
                            ui.selectable_value(&mut chat.send_channel, ChatChannel::Private, "PM");
                        });
                    if chat.send_channel == ChatChannel::Private {
                        ui.add(
                            egui::TextEdit::singleline(&mut chat.pm_target)
                                .desired_width(80.0)
                                .hint_text("to"),
                        );
                    }
                    let resp = ui.add(
                        egui::TextEdit::singleline(&mut chat.input)
                            .desired_width(ui.available_width() - 4.0)
                            .hint_text("Press Enter to chat"),
                    );
                    let submit = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    if submit && !chat.input.trim().is_empty() {
                        let text = chat.input.trim().to_string();
                        chat.input.clear();
                        out = Some(if chat.send_channel == ChatChannel::Private {
                            ChatOutput::Whisper(chat.pm_target.clone(), text)
                        } else {
                            ChatOutput::Say(chat.send_channel, text)
                        });
                        resp.request_focus();
                    }
                });
            });
        });
    out
}
