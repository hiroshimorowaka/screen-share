//! Unit tests for private `registry` helpers the behavioural suite in
//! `tests/registry.rs` can't reach through the public API.

use super::*;

/// Finding F04: `password_attempts_exceeded` used to prune only the
/// caller's own key, so a slow distributed brute force (one attempt per
/// source IP, keys never revisited) grew the map forever.
#[tokio::test(start_paused = true)]
async fn password_attempts_map_sweeps_stale_entries_instead_of_accumulating() {
    const DISTINCT_ATTACKERS: usize = 500;
    let mut attempts: HashMap<String, Vec<Instant>> = HashMap::new();

    for i in 0..DISTINCT_ATTACKERS {
        let key = format!("attacker-{i}");
        password_attempts_exceeded(&mut attempts, &key);
        // Mirror `join_room`'s own bookkeeping on a wrong password.
        attempts.entry(key).or_default().push(Instant::now());
    }
    assert_eq!(
        attempts.len(),
        DISTINCT_ATTACKERS,
        "each fresh attempt is tracked while its window is open"
    );

    // Every recorded attempt ages out of the window.
    tokio::time::advance(PASSWORD_ATTEMPT_WINDOW + Duration::from_secs(1)).await;

    // The next check from any client sweeps all of the stale keys.
    password_attempts_exceeded(&mut attempts, "attacker-fresh");
    assert!(
        attempts.is_empty(),
        "stale per-client entries are removed, not kept forever"
    );
}

/// The sweep must not evict a client whose attempts are still inside the
/// window — otherwise the lockout would never trip.
#[tokio::test(start_paused = true)]
async fn password_attempts_sweep_keeps_in_window_entries() {
    let mut attempts: HashMap<String, Vec<Instant>> = HashMap::new();

    // A client whose one attempt will age out of the window.
    attempts
        .entry("stale".to_string())
        .or_default()
        .push(Instant::now());
    tokio::time::advance(PASSWORD_ATTEMPT_WINDOW + Duration::from_secs(1)).await;

    for _ in 0..MAX_PASSWORD_ATTEMPTS {
        attempts
            .entry("attacker".to_string())
            .or_default()
            .push(Instant::now());
    }

    assert!(
        password_attempts_exceeded(&mut attempts, "attacker"),
        "the in-window attacker is still locked out"
    );
    assert_eq!(attempts.len(), 1, "only the stale client was swept");
}

/// P3 follow-up: `create_room` used the first `generate_room_code()` blind,
/// so a collision silently overwrote a live room.
#[test]
fn unique_room_code_skips_a_taken_code() {
    let mut seq = ["DUP00000", "DUP00000", "FREE1234"].into_iter();
    let code = unique_room_code(|c| c == "DUP00000", move || seq.next().unwrap().to_string());
    assert_eq!(code, "FREE1234");
}

#[test]
fn unique_room_code_returns_the_first_free_code_without_extra_tries() {
    let mut calls = 0;
    let code = unique_room_code(
        |_| false,
        || {
            calls += 1;
            "FIRST000".to_string()
        },
    );
    assert_eq!(code, "FIRST000");
    assert_eq!(calls, 1, "no retry when the first code is free");
}

#[test]
fn unique_room_code_gives_up_after_the_retry_budget() {
    let mut calls = 0;
    let code = unique_room_code(
        |_| true, // every code is "taken"
        || {
            calls += 1;
            format!("CODE{calls:04}")
        },
    );
    assert_eq!(calls, ROOM_CODE_COLLISION_RETRIES);
    assert_eq!(code, format!("CODE{ROOM_CODE_COLLISION_RETRIES:04}"));
}
