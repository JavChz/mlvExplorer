pub mod app;
pub mod audio;
pub mod config;
pub mod db;
pub mod gpu;
pub mod importer;
pub mod launcher;
pub mod mlv;
pub mod playback;
pub mod thumbnails;
pub mod ui;
pub mod updater;

use app::MlvExplorerApp;
use eframe::egui;

fn main() -> eframe::Result<()> {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("MLV Explorer · Magic Lantern RAW Library & Player")
            .with_inner_size([1360.0, 840.0])
            .with_min_inner_size([960.0, 600.0])
            .with_drag_and_drop(true),
        wgpu_options: eframe::egui_wgpu::WgpuConfiguration {
            power_preference: eframe::wgpu::PowerPreference::HighPerformance,
            ..Default::default()
        },
        ..Default::default()
    };

    eframe::run_native(
        "MLV Explorer",
        native_options,
        Box::new(|cc| Ok(Box::new(MlvExplorerApp::new(cc)))),
    )
}
