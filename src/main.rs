mod history;
mod i18n;
mod server;
mod slssteam;
mod steam;
mod ui;
mod updater;

use eframe::egui;

fn main() -> eframe::Result<()> {
    let mut viewport = egui::ViewportBuilder::default()
        .with_app_id("steamrollbackr")
        .with_inner_size([880.0, 620.0])
        .with_min_inner_size([720.0, 520.0]);

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

