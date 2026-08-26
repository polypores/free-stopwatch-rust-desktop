//! Application state and window-mode plumbing.
//!
//! UI drawing lives in `ui.rs`; timing logic lives in `stopwatch.rs`.
//! This file only owns state and translates user intent (toggle dark
//! mode, toggle always-on-top, ...) into `egui::ViewportCommand`s sent
//! to the OS window.

use std::time::Duration;

use crate::stopwatch::Stopwatch;
use crate::ui;

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum ViewMode {
    /// Full window: toolbar, big timer, buttons, lap list.
    Normal,
    /// Tiny borderless always-on-top strip, just the time and a toggle button.
    Compact,
}

pub struct StopwatchApp {
    pub stopwatch: Stopwatch,
    pub dark_mode: bool,
    pub always_on_top: bool,
    pub fullscreen: bool,
    pub view_mode: ViewMode,
}

// Width needs to comfortably fit the Start/Lap/Reset button row (see
// ui::BUTTON_SIZE / BUTTON_SPACING / BUTTON_PADDING): 3 buttons x 80px +
// 2 gaps x 12px + 2 x 12px frame padding + ~16px of CentralPanel margin
// = ~304px. 320 leaves a little breathing room. Using this for the
// *initial* size (not just the min) is what fixes the reset button being
// clipped the first time the app opens.
pub const NORMAL_SIZE: egui::Vec2 = egui::vec2(320.0, 340.0);
const COMPACT_SIZE: egui::Vec2 = egui::vec2(170.0, 60.0);

/// Absolute floor for the window. Small enough to allow Compact mode,
/// which draws a completely different (much narrower) layout and so
/// isn't subject to the button-row width threshold below.
pub const MIN_SIZE: egui::Vec2 = egui::vec2(140.0, 90.0);

/// Minimum size while in Normal mode. Deliberately matches `NORMAL_SIZE`'s
/// width so the button row can never be clipped by dragging the window
/// narrower — this is the horizontal-resize threshold. Applied dynamically
/// in `toggle_compact` (and set as the startup min in `main.rs`) rather
/// than baked into `MIN_SIZE`, since `MIN_SIZE` also has to accommodate
/// the much smaller Compact layout.
pub const NORMAL_MIN_SIZE: egui::Vec2 = egui::vec2(320.0, 90.0);

impl StopwatchApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Default to whatever the OS/egui already resolved (follows
        // Windows 11's light/dark setting on first launch).
        let dark_mode = cc.egui_ctx.style().visuals.dark_mode;
        Self {
            stopwatch: Stopwatch::default(),
            dark_mode,
            always_on_top: false,
            fullscreen: false,
            view_mode: ViewMode::Normal,
        }
    }

    pub fn toggle_dark_mode(&mut self) {
        self.dark_mode = !self.dark_mode;
    }

    pub fn toggle_always_on_top(&mut self, ctx: &egui::Context) {
        self.always_on_top = !self.always_on_top;
        self.apply_window_level(ctx);
    }

    fn apply_window_level(&self, ctx: &egui::Context) {
        // Compact mode is always pinned on top regardless of the toggle,
        // since a floating widget that can vanish behind other windows
        // defeats its own purpose.
        let on_top = self.always_on_top || self.view_mode == ViewMode::Compact;
        let level = if on_top {
            egui::WindowLevel::AlwaysOnTop
        } else {
            egui::WindowLevel::Normal
        };
        ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(level));
    }

    pub fn toggle_fullscreen(&mut self, ctx: &egui::Context) {
        self.fullscreen = !self.fullscreen;
        ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(self.fullscreen));
    }

    pub fn toggle_compact(&mut self, ctx: &egui::Context) {
        self.view_mode = match self.view_mode {
            ViewMode::Normal => ViewMode::Compact,
            ViewMode::Compact => ViewMode::Normal,
        };

        let (decorations, size, min_size) = match self.view_mode {
            ViewMode::Compact => (false, COMPACT_SIZE, MIN_SIZE),
            ViewMode::Normal => (true, NORMAL_SIZE, NORMAL_MIN_SIZE),
        };
        ctx.send_viewport_cmd(egui::ViewportCommand::Decorations(decorations));
        // Update the min-size floor *before* resizing, so the window
        // manager doesn't clamp the resize against the previous mode's
        // (wrong) floor.
        ctx.send_viewport_cmd(egui::ViewportCommand::MinInnerSize(min_size));
        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(size));
        self.apply_window_level(ctx);
    }
}

impl eframe::App for StopwatchApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // The single biggest CPU/RAM lever in an immediate-mode GUI: only
        // ask for the next frame when something is actually changing.
        // While running we need ~centisecond resolution on screen, so we
        // repaint at roughly 33fps; while stopped, egui only redraws in
        // response to input (clicks, resize, etc.), so idle CPU use is
        // effectively zero.
        if self.stopwatch.is_running() {
            ctx.request_repaint_after(Duration::from_millis(30));
        }

        ctx.set_visuals(if self.dark_mode {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        });

        ui::draw(ctx, self);
    }
}