//! Unit tests for `ws` — kept in-crate (they exercise the private
//! `over_rate_limit` helper) but split out of src/ws.rs to keep it
//! readable, matching the pattern used by `turn_tests.rs`. The handshake
//! `Origin` / client-key rules have their own tests in `handshake_tests.rs`.

use std::collections::VecDeque;

use tokio::time::Instant;

use super::{over_rate_limit, MAX_MSGS_PER_WINDOW, RATE_WINDOW};

#[tokio::test(start_paused = true)]
async fn over_rate_limit_trips_only_past_the_window_budget() {
    let mut recent = VecDeque::new();
    let start = Instant::now();

    // Exactly MAX_MSGS_PER_WINDOW messages in the window are allowed.
    for _ in 0..MAX_MSGS_PER_WINDOW {
        assert!(!over_rate_limit(&mut recent, start));
    }
    // One more within the same window trips it.
    assert!(over_rate_limit(&mut recent, start));
}

#[tokio::test(start_paused = true)]
async fn over_rate_limit_forgets_messages_older_than_the_window() {
    let mut recent = VecDeque::new();
    let start = Instant::now();

    for _ in 0..MAX_MSGS_PER_WINDOW {
        assert!(!over_rate_limit(&mut recent, start));
    }

    // A message a full window later slides all the earlier ones out, so
    // the budget is available again.
    let later = start + RATE_WINDOW;
    assert!(!over_rate_limit(&mut recent, later));
    assert_eq!(recent.len(), 1);
}
