// Suppresses the console window on Windows release builds (GUI app has no
// need for a terminal). Debug builds keep the console so you can see panics/logs.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod stopwatch;
mod ui;

use app::StopwatchApp;

fn main() -> eframe::Result<()> {
    // Start in Normal mode, so both the initial size and the resize floor
    // use app::NORMAL_SIZE / app::NORMAL_MIN_SIZE (wide enough for the
    // Start/Lap/Reset button row) rather than the smaller app::MIN_SIZE,
    // which exists only to let Compact mode shrink further.
    let viewport = egui::ViewportBuilder::default()
        .with_inner_size([app::NORMAL_SIZE.x, app::NORMAL_SIZE.y])
        .with_min_inner_size([app::NORMAL_MIN_SIZE.x, app::NORMAL_MIN_SIZE.y])
        .with_resizable(true)
        .with_title("Stopwatch");

    let native_options = eframe::NativeOptions {
        viewport,
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };

    eframe::run_native(
        "Stopwatch",
        native_options,
        Box::new(|cc| Ok(Box::new(StopwatchApp::new(cc)))),
    )
}