//! Pure timing logic for the stopwatch.
//!
//! Deliberately has zero dependency on egui/eframe: it can be unit-tested
//! on its own, and swapping the UI layer later (or adding a CLI mode)
//! wouldn't require touching this file.

use std::time::{Duration, Instant};

#[derive(Default, PartialEq, Eq, Clone, Copy)]
enum State {
    #[default]
    Stopped,
    Running,
}

/// A single lap: the elapsed-time window it covers, from the end of the
/// previous lap (or 00:00.00 for the first lap) to when this lap button
/// was pressed. Storing both ends (rather than just a split time) is
/// what lets the UI show "how long was this specific lap" without the
/// viewer having to subtract two rows in their head.
#[derive(Clone, Copy)]
pub struct Lap {
    /// Elapsed time when this lap began (= previous lap's `end`, or zero).
    pub start: Duration,
    /// Elapsed time when this lap was recorded.
    pub end: Duration,
}

impl Lap {
    /// Length of just this lap (`end - start`). `end >= start` always
    /// holds since both are samples of the same monotonically
    /// increasing `elapsed()` clock.
    pub fn duration(&self) -> Duration {
        self.end - self.start
    }
}

#[derive(Default)]
pub struct Stopwatch {
    state: State,
    /// Time accumulated from previous start/stop cycles.
    elapsed_before: Duration,
    /// When the current run started, if running.
    started_at: Option<Instant>,
    laps: Vec<Lap>,
}

impl Stopwatch {
    pub fn is_running(&self) -> bool {
        self.state == State::Running
    }

    pub fn start(&mut self) {
        if self.state == State::Stopped {
            self.started_at = Some(Instant::now());
            self.state = State::Running;
        }
    }

    pub fn stop(&mut self) {
        if self.state == State::Running {
            if let Some(t) = self.started_at.take() {
                self.elapsed_before += t.elapsed();
            }
            self.state = State::Stopped;
        }
    }

    pub fn toggle(&mut self) {
        if self.is_running() {
            self.stop();
        } else {
            self.start();
        }
    }

    pub fn reset(&mut self) {
        self.state = State::Stopped;
        self.elapsed_before = Duration::ZERO;
        self.started_at = None;
        self.laps.clear();
    }

    pub fn lap(&mut self) {
        if self.is_running() {
            let end = self.elapsed();
            // Each lap picks up where the previous one left off, so the
            // list reads as contiguous, non-overlapping segments.
            let start = self.laps.last().map_or(Duration::ZERO, |l| l.end);
            self.laps.push(Lap { start, end });
        }
    }

    pub fn elapsed(&self) -> Duration {
        self.elapsed_before + self.started_at.map_or(Duration::ZERO, |t| t.elapsed())
    }

    pub fn laps(&self) -> &[Lap] {
        &self.laps
    }
}

/// Formats a duration as `MM:SS.CC` (centiseconds), matching the
/// requested `00:00.00` display style.
pub fn format_time(d: Duration) -> String {
    let total_cs = d.as_millis() / 10;
    let cs = total_cs % 100;
    let total_secs = total_cs / 100;
    let secs = total_secs % 60;
    let mins = total_secs / 60;
    format!("{mins:02}:{secs:02}.{cs:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_zero() {
        assert_eq!(format_time(Duration::ZERO), "00:00.00");
    }

    #[test]
    fn formats_over_a_minute() {
        assert_eq!(format_time(Duration::from_millis(65_430)), "01:05.43");
    }

    #[test]
    fn start_stop_accumulates() {
        let mut sw = Stopwatch::default();
        assert!(!sw.is_running());
        sw.start();
        assert!(sw.is_running());
        std::thread::sleep(Duration::from_millis(20));
        sw.stop();
        assert!(!sw.is_running());
        assert!(sw.elapsed() >= Duration::from_millis(20));
    }

    #[test]
    fn lap_only_records_while_running() {
        let mut sw = Stopwatch::default();
        sw.lap(); // no-op, not running
        assert!(sw.laps().is_empty());
        sw.start();
        sw.lap();
        assert_eq!(sw.laps().len(), 1);
    }

    #[test]
    fn laps_are_contiguous_start_end_windows() {
        let mut sw = Stopwatch::default();
        sw.start();
        std::thread::sleep(Duration::from_millis(15));
        sw.lap();
        std::thread::sleep(Duration::from_millis(15));
        sw.lap();

        let laps = sw.laps();
        assert_eq!(laps.len(), 2);
        // First lap starts at zero.
        assert_eq!(laps[0].start, Duration::ZERO);
        // Second lap picks up exactly where the first one ended.
        assert_eq!(laps[1].start, laps[0].end);
        // Each lap's window is non-negative and non-trivial.
        assert!(laps[0].duration() > Duration::ZERO);
        assert!(laps[1].duration() > Duration::ZERO);
    }

    #[test]
    fn reset_clears_everything() {
        let mut sw = Stopwatch::default();
        sw.start();
        sw.lap();
        sw.reset();
        assert!(!sw.is_running());
        assert_eq!(sw.elapsed(), Duration::ZERO);
        assert!(sw.laps().is_empty());
    }
}