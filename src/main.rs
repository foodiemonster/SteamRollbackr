mod history;
mod server;
mod steam;
mod ui;

use eframe::egui;

fn main() -> eframe::Result<()> {
    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([760.0, 620.0])
        .with_min_inner_size([600.0, 480.0]);

    if let Ok(icon) = eframe::icon_data::from_png_bytes(include_bytes!("../packaging/steamrollbackr.png")) {
        viewport = viewport.with_icon(icon);
    }

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "SteamRollbackr",
        options,
        Box::new(|cc| Ok(Box::new(ui::SteamRollbackrApp::new(cc)))),
    )
}

