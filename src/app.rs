//! Application state and window-mode plumbing.
//!
//! UI drawing lives in `ui.rs`; timing logic lives in `stopwatch.rs`.
//! This file only owns state and translates user intent (toggle dark
//! mode, toggle always-on-top, ...) into `egui::ViewportCommand`s sent
//! to the OS window.

use std::time::Duration;

use eframe::Storage;

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
    /// Top-left corner the window last sat at in Normal mode, so
    /// switching back to Normal (this run, or restored from disk next
    /// launch) reopens in the same spot instead of wherever the OS feels
    /// like placing a fresh window. `None` until we've actually seen the
    /// window's position at least once.
    last_normal_pos: Option<egui::Pos2>,
    /// Same idea as `last_normal_pos`, but for the floating Compact strip
    /// — tracked separately since the two modes are almost always parked
    /// in different spots on screen (e.g. the toolbar corner vs. next to
    /// something being timed).
    last_compact_pos: Option<egui::Pos2>,
}

// Width needs to comfortably fit the Start/Stop + Lap/Reset button row
// (see ui::BUTTON_SIZE / BUTTON_SPACING / BUTTON_PADDING): 2 buttons x
// 80px + 1 gap x 12px + 2 x 12px frame padding + ~16px of CentralPanel
// margin = ~212px. 320 is kept (rather than shrunk to match) so the
// window doesn't get cramped now that there's only one row of controls,
// and so button_scale (which scales off a 320px reference) still has
// room to grow the buttons on a wider window. Using this for the
// *initial* size (not just the min) is what fixes buttons being clipped
// the first time the app opens.
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

// Storage keys for the position-persistence in `save`/`new` below.
// NOTE: this relies on eframe's on-disk `Storage` backend, which lives
// behind eframe's `persistence` Cargo feature. If that feature isn't
// already enabled in Cargo.toml, `cc.storage` is always `None` and
// positions will still be remembered for the lifetime of the process
// (via `last_normal_pos`/`last_compact_pos`) but won't survive a restart.
const NORMAL_POS_KEY: &str = "normal_pos";
const COMPACT_POS_KEY: &str = "compact_pos";

/// Parses the `"x,y"` strings written by `save` below. Split out mainly
/// so a corrupt/foreign value in storage degrades to "forget it" instead
/// of panicking.
fn parse_pos(value: Option<String>) -> Option<egui::Pos2> {
    let value = value?;
    let (x, y) = value.split_once(',')?;
    Some(egui::pos2(x.parse().ok()?, y.parse().ok()?))
}

fn format_pos(pos: egui::Pos2) -> String {
    format!("{},{}", pos.x, pos.y)
}

impl StopwatchApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Default to whatever the OS/egui already resolved (follows
        // Windows 11's light/dark setting on first launch).
        let dark_mode = cc.egui_ctx.style().visuals.dark_mode;

        let last_normal_pos = cc
            .storage
            .and_then(|s| parse_pos(s.get_string(NORMAL_POS_KEY)));
        let last_compact_pos = cc
            .storage
            .and_then(|s| parse_pos(s.get_string(COMPACT_POS_KEY)));

        // The window itself is already created (by main.rs's
        // ViewportBuilder) at its default OS-chosen position by the time
        // we get here, always in Normal mode. If we have a remembered
        // Normal-mode spot, move it there right away rather than waiting
        // for the user to notice and re-drag it every launch.
        if let Some(pos) = last_normal_pos {
            cc.egui_ctx
                .send_viewport_cmd(egui::ViewportCommand::OuterPosition(pos));
        }

        Self {
            stopwatch: Stopwatch::default(),
            dark_mode,
            always_on_top: false,
            fullscreen: false,
            view_mode: ViewMode::Normal,
            last_normal_pos,
            last_compact_pos,
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

        let (decorations, resizable, size, min_size) = match self.view_mode {
            ViewMode::Compact => (false, false, COMPACT_SIZE, MIN_SIZE),
            ViewMode::Normal => (true, true, NORMAL_SIZE, NORMAL_MIN_SIZE),
        };
        ctx.send_viewport_cmd(egui::ViewportCommand::Decorations(decorations));
        // Windows only offers Snap Assist (the edge/corner drag-to-quarter
        // behavior) to windows it considers resizable. Compact is a fixed-
        // size strip, so marking it non-resizable stops the OS from ever
        // proposing a quarter-screen snap in the first place — this is the
        // primary fix for dragging the floating window into a screen
        // corner. `enforce_compact_size` (below) is kept as a fallback in
        // case some platform/WM still offers a snap despite this.
        ctx.send_viewport_cmd(egui::ViewportCommand::Resizable(resizable));
        // Update the min-size floor *before* resizing, so the window
        // manager doesn't clamp the resize against the previous mode's
        // (wrong) floor.
        ctx.send_viewport_cmd(egui::ViewportCommand::MinInnerSize(min_size));
        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(size));

        // Drop the window back wherever it last sat *in the mode we're
        // switching into* — Normal and Compact are tracked separately
        // since they're usually parked in different places.
        let remembered_pos = match self.view_mode {
            ViewMode::Normal => self.last_normal_pos,
            ViewMode::Compact => self.last_compact_pos,
        };
        if let Some(pos) = remembered_pos {
            ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(pos));
        }

        self.apply_window_level(ctx);
    }

    /// Records where the window currently sits, keyed by whichever mode
    /// is active. Called every frame from `update` — cheap (just an
    /// `Option<Pos2>` write) and means we never need a dedicated "drag
    /// ended" hook to know the final position, including drags done via
    /// the OS's own title bar in Normal mode (which we don't get an egui
    /// callback for at all).
    fn track_window_pos(&mut self, ctx: &egui::Context) {
        // Fullscreen reports a position/size that isn't a meaningful
        // "restore point" (it's the whole monitor), so don't let it
        // clobber the last real position.
        if self.fullscreen {
            return;
        }
        let Some(rect) = ctx.input(|i| i.viewport().outer_rect) else {
            return;
        };
        match self.view_mode {
            ViewMode::Normal => self.last_normal_pos = Some(rect.min),
            ViewMode::Compact => self.last_compact_pos = Some(rect.min),
        }
    }

    /// Undoes Windows' Snap Assist the moment it fires. Compact mode drags
    /// through the OS's native window-move (see `ui::draw_compact`) so it
    /// tracks the mouse instantly with no blur/lag — but that's the exact
    /// mechanism Snap Assist hooks into: drag near a screen edge or corner
    /// and Windows briefly inflates the window to a half/quarter of the
    /// screen. Rather than avoiding native drag (which traded that problem
    /// for a worse one — see the comment in `ui::draw_compact`), just watch
    /// for the inflated size every frame and snap it straight back to
    /// `COMPACT_SIZE`. The window ends up parked wherever it was dropped —
    /// including right in the corner — instead of taking up a quarter of
    /// the screen.
    fn enforce_compact_size(&self, ctx: &egui::Context) {
        if self.view_mode != ViewMode::Compact {
            return;
        }

        let maximized = ctx.input(|i| i.viewport().maximized).unwrap_or(false);
        let wrong_size = ctx.input(|i| i.viewport().inner_rect).is_some_and(|rect| {
            let size = rect.size();
            (size.x - COMPACT_SIZE.x).abs() > 1.0 || (size.y - COMPACT_SIZE.y).abs() > 1.0
        });

        if !maximized && !wrong_size {
            return;
        }

        if maximized {
            ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(false));
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::MinInnerSize(MIN_SIZE));
        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(COMPACT_SIZE));
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

        self.track_window_pos(ctx);
        self.enforce_compact_size(ctx);

        ctx.set_visuals(if self.dark_mode {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        });

        ui::draw(ctx, self);
    }

    // eframe's default clear_color is a fixed near-black, independent of
    // the app's theme. Left as-is, that near-black can show through
    // anywhere a panel doesn't paint over it (e.g. rounded corners on the
    // borderless Compact window) — invisible-on-invisible in light mode,
    // same root cause as the Compact-frame fix in ui::draw_compact. Tie
    // it to the current visuals instead.
    fn clear_color(&self, visuals: &egui::Visuals) -> [f32; 4] {
        visuals.panel_fill.to_normalized_gamma_f32()
    }

    // Persists the last-seen window position for each mode to disk (via
    // eframe's Storage backend) so it survives an app restart, not just
    // mode-switches within a single run. eframe calls this periodically
    // and on shutdown.
    fn save(&mut self, storage: &mut dyn Storage) {
        if let Some(pos) = self.last_normal_pos {
            storage.set_string(NORMAL_POS_KEY, format_pos(pos));
        }
        if let Some(pos) = self.last_compact_pos {
            storage.set_string(COMPACT_POS_KEY, format_pos(pos));
        }
    }
}