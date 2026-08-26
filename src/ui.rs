//! Immediate-mode UI drawing. No app state lives here — everything is
//! read from / written to `StopwatchApp`, so this file can be reshuffled
//! or restyled without touching timing or window logic.

use crate::app::{StopwatchApp, ViewMode};
use crate::stopwatch::format_time;

pub fn draw(ctx: &egui::Context, app: &mut StopwatchApp) {
    match app.view_mode {
        ViewMode::Compact => draw_compact(ctx, app),
        ViewMode::Normal => draw_normal(ctx, app),
    }
}

fn draw_normal(ctx: &egui::Context, app: &mut StopwatchApp) {
    egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            toolbar_toggles(ui, app, ctx);
        });
        ui.add_space(4.0);
    });

    egui::CentralPanel::default().show(ctx, |ui| {
        ui.vertical_centered(|ui| {
            ui.add_space(16.0);
            time_label(ui, app, 46.0);
            ui.add_space(12.0);
            control_buttons(ui, app);
            ui.add_space(10.0);
            ui.separator();
            laps_list(ui, app);
        });
    });
}

fn draw_compact(ctx: &egui::Context, app: &mut StopwatchApp) {
    egui::CentralPanel::default()
        // Start from the *real* central-panel frame for the current style
        // (which fills in visuals.panel_fill) rather than
        // Frame::default(), whose fill is transparent. With a transparent
        // fill nothing paints over eframe's own default clear color (a
        // near-black), which is invisible-on-invisible with light mode's
        // dark text — exactly the "can't see anything" bug in Compact/
        // floating mode.
        .frame(egui::Frame::central_panel(&ctx.style()).inner_margin(6.0))
        .show(ctx, |ui| {
            // Compact mode removes the native window decorations, so there is
            // no title bar to drag. Make the client area a native window drag
            // region. The buttons below are added afterwards and keep their
            // normal click behavior.
            //
            // This is the OS's own native drag (HTCAPTION-style on Windows),
            // not a manual per-frame reposition. A manual approach was tried
            // to dodge Windows' Snap Assist (see below), but it fights the
            // desktop compositor's own move-smoothing: DWM doesn't treat a
            // rapid stream of programmatic position commands as a live
            // interactive move the way it does a native drag, so the window
            // visibly blurred and lagged behind the mouse instead of
            // tracking it 1:1. Native drag has neither problem. Snap Assist
            // (drag near a screen edge/corner briefly inflates the window to
            // a half/quarter of the screen) is instead corrected after the
            // fact — see `StopwatchApp::enforce_compact_size`, called every
            // frame in `update`, which snaps the size straight back the
            // moment it notices Snap Assist has changed it.
            let drag_id = ui.make_persistent_id("compact_window_drag");
            let drag_response = ui.interact(ui.max_rect(), drag_id, egui::Sense::drag());

            if drag_response.drag_started() {
                ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
            }

            ui.horizontal_centered(|ui| {
                time_label(ui, app, 20.0);
                ui.add_space(6.0);
                let icon = if app.stopwatch.is_running() { "⏸" } else { "▶" };
                if ui.small_button(icon).clicked() {
                    app.stopwatch.toggle();
                }
                if ui
                    .small_button("⤢")
                    .on_hover_text("Exit floating mode")
                    .clicked()
                {
                    app.toggle_compact(ctx);
                }
            });
        });
}

fn time_label(ui: &mut egui::Ui, app: &StopwatchApp, size: f32) {
    ui.label(
        egui::RichText::new(format_time(app.stopwatch.elapsed()))
            .monospace()
            .size(size)
            .strong(),
    );
}

// pub(crate): app::NORMAL_MIN_SIZE / NORMAL_SIZE are sized off of these,
// to keep the control-button row from ever being clipped horizontally.
pub(crate) const BUTTON_SIZE: egui::Vec2 = egui::vec2(80.0, 36.0);
pub(crate) const BUTTON_SPACING: f32 = 12.0;
pub(crate) const BUTTON_PADDING: egui::Vec2 = egui::vec2(12.0, 8.0);
const LABEL_FONT_SIZE: f32 = 16.0;

/// Size the control row from the actual content area. The width is the
/// primary driver, while the height caps the scale so a very wide but short
/// window cannot make the buttons too tall for the layout.
fn button_scale(ui: &egui::Ui) -> f32 {
    let available = ui.available_size();
    let width_scale = available.x / 320.0;
    let height_scale = available.y / 140.0;

    width_scale.min(height_scale).clamp(1.0, 3.0)
}

fn control_buttons(ui: &mut egui::Ui, app: &mut StopwatchApp) {
    let scale = button_scale(ui);
    let button_size = BUTTON_SIZE * scale;
    let button_spacing = BUTTON_SPACING * scale;
    let vertical_padding = BUTTON_PADDING.y * scale;
    let label_font_size = LABEL_FONT_SIZE * scale;

    // Reserve the full available width. This is important: a plain
    // horizontal() only sizes itself to its contents, which makes the row
    // appear stuck to the left when the window gets wider.
    let available_width = ui.available_width();
    let row_height = button_size.y + (vertical_padding * 2.0);

    // Only two buttons now: Start/Stop, and a second slot that doubles as
    // Lap (while running) or Reset (while stopped) — see secondary_button.
    ui.allocate_ui_with_layout(
        egui::vec2(available_width, row_height),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            ui.add_space(
                (available_width
                    - (button_size.x * 2.0 + button_spacing))
                    .max(0.0)
                    / 2.0,
            );

            ui.spacing_mut().item_spacing.x = button_spacing;

            let running = app.stopwatch.is_running();
            let label = if running { "⏸ Stop" } else { "▶ Start" };
            toggle_button(ui, app, label, button_size, label_font_size);
            secondary_button(ui, app, button_size, label_font_size);
        },
    );
}

fn styled_button(
    ui: &mut egui::Ui,
    text: &str,
    button_size: egui::Vec2,
    label_font_size: f32,
) -> egui::Response {
    ui.add_sized(
        button_size,
        egui::Button::new(egui::RichText::new(text).size(label_font_size)),
    )
}

fn toggle_button(
    ui: &mut egui::Ui,
    app: &mut StopwatchApp,
    label: &str,
    button_size: egui::Vec2,
    label_font_size: f32,
) {
    if styled_button(ui, label, button_size, label_font_size).clicked() {
        app.stopwatch.toggle();
    }
}

/// Second control-row button. It's a single slot that changes both its
/// label/icon and its behavior depending on run state, rather than two
/// buttons that swap enabled/disabled: while running it's "Lap" (record a
/// split), and while stopped it's "Reset" (clear the stopwatch). This
/// keeps the row at two buttons total instead of three.
fn secondary_button(
    ui: &mut egui::Ui,
    app: &mut StopwatchApp,
    button_size: egui::Vec2,
    label_font_size: f32,
) {
    let running = app.stopwatch.is_running();
    let label = if running { "⏱ Lap" } else { "↺ Reset" };
    if styled_button(ui, label, button_size, label_font_size).clicked() {
        if running {
            app.stopwatch.lap();
        } else {
            app.stopwatch.reset();
        }
    }
}

fn laps_list(ui: &mut egui::Ui, app: &StopwatchApp) {
    let laps = app.stopwatch.laps();

    if laps.is_empty() {
        ui.add_space(8.0);
        ui.weak("No laps yet");
        return;
    }

    egui::ScrollArea::vertical().max_height(160.0).show(ui, |ui| {
        // A Grid (rather than one label per row) is what keeps the four
        // columns vertically aligned as the numbers change width-per-row
        // — plain string formatting with padding drifts out of alignment
        // once a proportional-ish monospace mix or a 3-digit lap count
        // shows up, a Grid measures each column and never does.
        egui::Grid::new("laps_grid")
            .num_columns(4)
            .striped(true)
            .spacing([16.0, 4.0])
            .show(ui, |ui| {
                ui.strong("Lap");
                ui.strong("Start");
                ui.strong("End");
                ui.strong("Duration");
                ui.end_row();

                // Newest lap on top, like the old behavior.
                for (i, lap) in laps.iter().enumerate().rev() {
                    ui.label(format!("{}", i + 1));
                    ui.monospace(format_time(lap.start));
                    ui.monospace(format_time(lap.end));
                    ui.monospace(format_time(lap.duration()));
                    ui.end_row();
                }
            });
    });
}

fn toolbar_toggles(ui: &mut egui::Ui, app: &mut StopwatchApp, ctx: &egui::Context) {
    if ui
        .selectable_label(app.dark_mode, if app.dark_mode { "🌙" } else { "☀" })
        .on_hover_text("Light / dark mode")
        .clicked()
    {
        app.toggle_dark_mode();
    }
    if ui
        .selectable_label(app.always_on_top, "📌")
        .on_hover_text("Always on top")
        .clicked()
    {
        app.toggle_always_on_top(ctx);
    }
    if ui
        .selectable_label(app.fullscreen, "⛶")
        .on_hover_text("Fullscreen")
        .clicked()
    {
        app.toggle_fullscreen(ctx);
    }
    if ui
        .button("▫ Float")
        .on_hover_text("Small floating always-on-top window")
        .clicked()
    {
        app.toggle_compact(ctx);
    }
}