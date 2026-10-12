//! Explorer — a native, cross-platform file manager.
//!
//! Usage: `explorer [PATH]`

use std::path::PathBuf;

fn main() -> eframe::Result<()> {
    let start = std::env::args_os().nth(1).map(PathBuf::from);
    if let Some(p) = &start {
        if !p.exists() {
            eprintln!("explorer: {}: no such file or directory", p.display());
        }
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1180.0, 760.0])
            .with_min_inner_size([620.0, 380.0])
            .with_title("Explorer")
            .with_app_id("dev.explorer.Explorer"),
        ..Default::default()
    };

    eframe::run_native(
        "Explorer",
        options,
        Box::new(move |cc| Ok(Box::new(explorer_ui::App::new(cc, start.clone())))),
    )
}
