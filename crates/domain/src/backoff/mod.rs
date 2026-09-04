//! The reconnect backoff schedule: how long to wait before each rejoin
//! attempt after the signaling WebSocket drops, and when to give up.
//!
//! Pure and deterministic given the injected jitter fraction — the
//! `hydrate`-only wiring that samples `Math.random`, sleeps, and reopens
//! the socket lives in `apps/web`'s `session::reconnect`.

/// First retry waits about this long. Short — most drops are brief.
const BASE_DELAY_MS: u32 = 1_000;
/// Backoff never waits longer than this between attempts.
const MAX_DELAY_MS: u32 = 20_000;
/// Give up after this many failed attempts (~1-2 min of trying with the
/// delays above) and fall back to asking the user to reload.
const MAX_ATTEMPTS: u32 = 8;

/// Exponential backoff with "half jitter" and a hard attempt cap. Pure and
/// deterministic given the jitter fraction, so the schedule is unit-tested
/// directly.
#[derive(Debug, Default)]
pub struct BackoffPolicy {
    attempts_made: u32,
}

impl BackoffPolicy {
    pub fn new() -> Self {
        Self::default()
    }

    /// The delay before the next attempt, or `None` once [`MAX_ATTEMPTS`]
    /// have been used. Advances the attempt counter.
    ///
    /// `jitter01` is a caller-supplied random fraction in `[0.0, 1.0)`
    /// (injected rather than sampled here to keep this testable). The
    /// returned delay lies in `[target/2, target)` where `target` is the
    /// uncapped-then-capped exponential value — spreading retries out so a
    /// whole room that dropped together doesn't reconnect in lockstep.
    pub fn next_delay_ms(&mut self, jitter01: f64) -> Option<u32> {
        if self.attempts_made >= MAX_ATTEMPTS {
            return None;
        }
        let exponential = BASE_DELAY_MS.saturating_mul(1u32 << self.attempts_made);
        let target = exponential.min(MAX_DELAY_MS);
        self.attempts_made += 1;

        let half = target / 2;
        let jittered = half as f64 + half as f64 * jitter01.clamp(0.0, 1.0);
        Some(jittered as u32)
    }

    /// Resets the counter after a reconnection succeeds, so a later drop
    /// starts its backoff from scratch.
    pub fn reset(&mut self) {
        self.attempts_made = 0;
    }

    pub fn attempts_made(&self) -> u32 {
        self.attempts_made
    }

    /// Total attempts the policy will make before giving up — for the
    /// "attempt N of M" status text.
    pub fn max_attempts() -> u32 {
        MAX_ATTEMPTS
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
