//! SPEC §14.2 — frame pacing.
//!
//! Pure, so it can be tested without a display: the player may not begin a new
//! logical frame sooner than `1/60` s of wall time after the previous one, and
//! presents exactly once per logical frame.

use std::time::Duration;

use rico16_core::limits::DT;

/// SPEC §5.1, §14.2 — the minimum wall-clock interval between logical frames.
pub fn frame_interval() -> Duration {
    Duration::from_secs_f64(DT)
}

/// A one-frame-per-interval gate.
#[derive(Debug, Clone, Default)]
pub struct FramePacer {
    last: Duration,
    started: bool,
}

impl FramePacer {
    /// A pacer that is ready immediately.
    pub fn new() -> Self {
        Self::default()
    }

    /// SPEC §14.2 — may a logical frame start at `now`?
    pub fn ready(&self, now: Duration) -> bool {
        !self.started || now.saturating_sub(self.last) >= frame_interval()
    }

    /// The earliest instant the next logical frame may start.
    pub fn deadline(&self) -> Duration {
        if self.started {
            self.last + frame_interval()
        } else {
            Duration::ZERO
        }
    }

    /// Record that a logical frame started at `now`.
    pub fn advance(&mut self, now: Duration) {
        self.last = now;
        self.started = true;
    }
}
