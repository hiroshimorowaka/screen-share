//! Unit tests for the private `RoomStatusLimiter` — split out of
//! src/rooms_status.rs like the other `*_tests.rs` files. The HTTP
//! handler itself is covered by `tests/rooms_status.rs`.

use super::{RoomStatusLimiter, MAX_ROOM_STATUS_REQUESTS, MAX_TRACKED_CLIENTS, ROOM_STATUS_WINDOW};

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

#[tokio::test(start_paused = true)]
async fn the_stale_client_sweep_only_fires_strictly_above_the_tracking_cap() {
    let limiter = RoomStatusLimiter::new();

    // Exactly MAX_TRACKED_CLIENTS distinct keys, then let them all go stale.
    for i in 0..MAX_TRACKED_CLIENTS {
        limiter.allow(&format!("k{i}"));
    }
    tokio::time::advance(ROOM_STATUS_WINDOW).await;

    // One more request: len == MAX_TRACKED_CLIENTS, which is NOT strictly
    // greater, so no sweep runs — the stale keys are still tracked.
    limiter.allow("straw");
    assert_eq!(
        limiter.tracked_clients(),
        MAX_TRACKED_CLIENTS + 1,
        "the sweep must not run when the map is exactly at the cap"
    );
}

#[tokio::test(start_paused = true)]
async fn the_sweep_drops_stale_only_clients_and_keeps_ones_with_a_recent_hit() {
    let limiter = RoomStatusLimiter::new();

    for i in 0..MAX_TRACKED_CLIENTS {
        limiter.allow(&format!("k{i}"));
    }

    // Halfway through the window, give "k0" a second, fresher timestamp.
    tokio::time::advance(ROOM_STATUS_WINDOW / 2).await;
    limiter.allow("k0");

    // Advance to exactly one window from the start: every original
    // timestamp is now at the stale edge, "k0"'s second one is still fresh.
    tokio::time::advance(ROOM_STATUS_WINDOW / 2).await;

    // "fresh" is added while len == MAX_TRACKED_CLIENTS (no sweep yet)...
    limiter.allow("fresh");
    // ...then this call pushes len above the cap and triggers the sweep.
    limiter.allow("trigger");

    assert_eq!(
        limiter.tracked_clients(),
        3,
        "sweep keeps only k0 (recent hit) and fresh, plus the triggering key"
    );
}
