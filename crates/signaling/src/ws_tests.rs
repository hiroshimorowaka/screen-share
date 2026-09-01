//! Unit tests for `ws` — kept in-crate (they exercise the private
//! `client_key` and `over_rate_limit` helpers) but split out of src/ws.rs
//! to keep it readable, matching the pattern used by `turn_tests.rs`.

use std::collections::VecDeque;

use axum::http::HeaderMap;
use tokio::time::Instant;

use super::{client_key, over_rate_limit, MAX_MSGS_PER_WINDOW, RATE_WINDOW};

#[test]
fn client_key_uses_the_fly_client_ip_header_when_present() {
    let mut headers = HeaderMap::new();
    headers.insert("fly-client-ip", "203.0.113.7".parse().unwrap());

    assert_eq!(client_key(&headers), "203.0.113.7");
}

#[test]
fn client_key_falls_back_to_a_constant_without_the_header() {
    assert_eq!(client_key(&HeaderMap::new()), "unknown");
}

#[test]
fn client_key_falls_back_when_the_header_is_not_valid_utf8() {
    let mut headers = HeaderMap::new();
    headers.insert(
        "fly-client-ip",
        axum::http::HeaderValue::from_bytes(&[0xff, 0xfe]).unwrap(),
    );

    assert_eq!(client_key(&headers), "unknown");
}

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
