//! Unit tests for the private `RoomStatusLimiter` — split out of
//! src/rooms_status.rs like the other `*_tests.rs` files. The HTTP
//! handler itself is covered by `tests/rooms_status.rs`.

use super::{RoomStatusLimiter, MAX_ROOM_STATUS_REQUESTS};

#[tokio::test(start_paused = true)]
async fn allows_up_to_the_budget_then_refuses_within_the_window() {
    let limiter = RoomStatusLimiter::new();

    for _ in 0..MAX_ROOM_STATUS_REQUESTS {
        assert!(limiter.allow("client-a"));
    }
    assert!(!limiter.allow("client-a"), "one past the budget is refused");

    // A different client has its own budget.
    assert!(limiter.allow("client-b"));
}

#[tokio::test(start_paused = true)]
async fn the_budget_refills_after_the_window_elapses() {
    let limiter = RoomStatusLimiter::new();

    for _ in 0..MAX_ROOM_STATUS_REQUESTS {
        assert!(limiter.allow("client-a"));
    }
    assert!(!limiter.allow("client-a"));

    tokio::time::advance(super::ROOM_STATUS_WINDOW).await;
    assert!(
        limiter.allow("client-a"),
        "requests older than the window no longer count"
    );
}
