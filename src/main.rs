pub mod app;
pub mod db;
pub mod mlv;
pub mod ui;

use app::MlvExplorerApp;
use eframe::egui;

fn main() -> eframe::Result<()> {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("MLV Explorer - Magic Lantern Video Library & Player")
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([900.0, 600.0])
            .with_drag_and_drop(true),
        ..Default::default()
    };

    eframe::run_native(
        "MLV Explorer",
        native_options,
        Box::new(|cc| Ok(Box::new(MlvExplorerApp::new(cc)))),
    )
}
