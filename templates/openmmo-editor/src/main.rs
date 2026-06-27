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
            ui.label("Scaffold — map, item, and NPC editors coming soon.");
            ui.separator();
            ui.label("Open a content pack directory to begin authoring.");
        });
    }
}
