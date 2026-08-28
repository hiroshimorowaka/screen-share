//! Unit tests for `member_card`, split out of src/member_card.rs to keep it
//! readable (refactor Phase 4). Kept in-crate: they exercise items that
//! are not part of the crate's public API.

use super::*;

#[test]
fn ping_color_var_classifies_into_three_tiers() {
    assert_eq!(ping_color_var(0), "--success");
    assert_eq!(ping_color_var(PING_GOOD_MS - 1), "--success");
    assert_eq!(ping_color_var(PING_GOOD_MS), "--warning");
    assert_eq!(ping_color_var(PING_WARN_MS - 1), "--warning");
    assert_eq!(ping_color_var(PING_WARN_MS), "--error");
    assert_eq!(ping_color_var(9999), "--error");
}
