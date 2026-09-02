//! Unit tests for `BackoffPolicy`.

use super::*;

/// Midpoint jitter — the returned delay is 3/4 of the capped target.
const MID: f64 = 0.5;

#[test]
fn first_attempt_waits_around_the_base_delay() {
    let mut policy = BackoffPolicy::new();
    let delay = policy.next_delay_ms(0.0).unwrap();
    // Half-jitter with fraction 0.0 gives exactly half the target.
    assert_eq!(delay, BASE_DELAY_MS / 2);
    assert_eq!(policy.attempts_made(), 1);
}

#[test]
fn jitter_fraction_spans_the_upper_half_of_the_target() {
    let low = BackoffPolicy::new().next_delay_ms(0.0).unwrap();
    let high = BackoffPolicy::new().next_delay_ms(0.999).unwrap();
    assert!(low < high);
    assert_eq!(low, BASE_DELAY_MS / 2);
    assert!(high < BASE_DELAY_MS, "stays below the full target");
    assert!(high >= BASE_DELAY_MS * 3 / 4);
}

#[test]
fn backoff_grows_then_flattens_at_the_ceiling() {
    let mut policy = BackoffPolicy::new();
    let mut delays = Vec::new();
    while let Some(delay) = policy.next_delay_ms(MID) {
        delays.push(delay);
    }

    assert!(
        delays.windows(2).all(|w| w[0] <= w[1]),
        "non-decreasing: {delays:?}"
    );
    assert!(
        delays.iter().all(|d| *d < MAX_DELAY_MS),
        "half-jitter keeps every delay under the ceiling: {delays:?}"
    );
    // Once the exponential passes the ceiling, every remaining delay is the
    // same capped value.
    let capped = *delays.last().unwrap();
    assert!(capped >= MAX_DELAY_MS / 2);
    assert!(
        delays.iter().rev().take(2).all(|d| *d == capped),
        "the tail is flat at the cap: {delays:?}"
    );
}

#[test]
fn gives_up_after_the_attempt_cap() {
    let mut policy = BackoffPolicy::new();
    let mut count = 0;
    while policy.next_delay_ms(MID).is_some() {
        count += 1;
        assert!(count <= 100, "must terminate");
    }
    assert_eq!(count, BackoffPolicy::max_attempts());
    assert_eq!(policy.next_delay_ms(MID), None, "stays given up");
}

#[test]
fn reset_restarts_the_schedule() {
    let mut policy = BackoffPolicy::new();
    while policy.next_delay_ms(MID).is_some() {}
    assert_eq!(policy.attempts_made(), BackoffPolicy::max_attempts());

    policy.reset();
    assert_eq!(policy.attempts_made(), 0);
    assert_eq!(policy.next_delay_ms(0.0), Some(BASE_DELAY_MS / 2));
}

#[test]
fn a_jitter_fraction_out_of_range_is_clamped() {
    // Below 0 clamps to 0 -> the lower bound (half the target).
    assert_eq!(
        BackoffPolicy::new().next_delay_ms(-3.0),
        Some(BASE_DELAY_MS / 2)
    );
    // Above 1 clamps to 1 -> the full target (real Math::random never
    // reaches this, it's just defensive).
    assert_eq!(BackoffPolicy::new().next_delay_ms(9.0), Some(BASE_DELAY_MS));
}
