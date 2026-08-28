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
