use anyhow::{anyhow, Result};
use app::SorterApp;
use eframe::egui;

mod app;

use crate::cli::args::Args;

pub fn run_gui(args: Args) -> Result<()> {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([900.0, 600.0])
            .with_min_inner_size([640.0, 400.0])
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
        Box::new(move |_cc| Ok(Box::new(SorterApp::new(args.source_dir, args.target_dir)))),
    )
    .map_err(|e| anyhow!("failed: {e:#}"))
}
