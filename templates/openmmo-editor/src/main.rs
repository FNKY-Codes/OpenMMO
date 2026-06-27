use eframe::egui;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "OpenMMO Editor",
        options,
        Box::new(|_cc| Ok(Box::new(EditorApp::default()))),
    )
}

#[derive(Default)]
struct EditorApp;

impl eframe::App for EditorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("OpenMMO Editor");
            ui.label("Visual content editor scaffold.");
            ui.separator();
            egui::CollapsingHeader::new("Items").show(ui, |ui| {
                ui.label("Item editor — define prowess, attack_ticks, tool tags.");
            });
            egui::CollapsingHeader::new("NPCs").show(ui, |ui| {
                ui.label("NPC editor — stats, loot tables, attack_ticks.");
            });
            egui::CollapsingHeader::new("Regions").show(ui, |ui| {
                ui.label("Map painter — tile placement and spawn points.");
            });
            if ui.button("Play test (connect to local server)").clicked() {
                ui.label("Launch openmmo-client against ws://127.0.0.1:8080/ws");
            }
        });
    }
}
