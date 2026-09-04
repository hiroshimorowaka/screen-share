//! Unit tests for `grid`, split out of src/grid.rs to keep it
//! readable (refactor Phase 4). Kept in-crate: they exercise items that
//! are not part of the crate's public API.

use super::*;

#[test]
fn best_column_count_picks_the_aspect_closest_to_16_9() {
    assert_eq!(best_column_count(1, 1920.0, 900.0), 1);
    assert_eq!(best_column_count(2, 1920.0, 900.0), 2);
    assert_eq!(best_column_count(3, 1920.0, 900.0), 2);
    assert_eq!(best_column_count(5, 1920.0, 900.0), 3);
    assert_eq!(best_column_count(10, 1920.0, 900.0), 4);
}

#[test]
fn best_column_count_never_exceeds_the_member_count() {
    for visible in 1..=10 {
        assert!(best_column_count(visible, 1920.0, 900.0) <= visible);
    }
}

#[test]
fn best_column_count_is_never_zero() {
    for visible in 1..=10 {
        assert!(best_column_count(visible, 1920.0, 900.0) >= 1);
    }
}

#[test]
fn best_column_count_forces_two_columns_on_a_narrow_portrait_screen() {
    // A 390x750 phone: aspect alone would pick 1 column for these counts.
    // One or two members still stack (two big tiles beat two skinny ones);
    // the clamp only kicks in once there are three or more.
    assert_eq!(best_column_count(1, 390.0, 750.0), 1);
    assert_eq!(best_column_count(2, 390.0, 750.0), 1);
    for visible in 3..=10 {
        assert!(
            best_column_count(visible, 390.0, 750.0) >= 2,
            "{visible} members on a narrow screen should not collapse to one column",
        );
    }
    // The clamp is width-gated — a wide viewport is unaffected.
    assert_eq!(best_column_count(3, 1920.0, 900.0), 2);
}
