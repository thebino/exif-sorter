use anyhow::{anyhow, Result};
use app::MyApp;
use eframe::egui;

mod app;

use crate::cli::args::Args;

pub fn run_gui(_args: Args) -> Result<()> {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([400.0, 300.0])
            .with_min_inner_size([300.0, 220.0])
            .with_icon(
                // NOTE: Adding an icon is optional
                eframe::icon_data::from_png_bytes(&include_bytes!("../../assets/icon-256.png")[..])
                    .expect("Failed to load icon"),
            ),
        ..Default::default()
    };
    eframe::run_native(
        "exif-sorter",
        native_options,
        Box::new(|_cc| Ok(Box::new(MyApp::default()))),
    )
    .map_err(|e| anyhow!("failed: {e:#}"))
}
