//! Unit tests for `palette`, split out of src/palette.rs to keep it
//! readable (refactor Phase 4). Kept in-crate: they exercise items that
//! are not part of the crate's public API.

use super::*;

#[test]
fn color_hex_returns_the_pair_for_a_known_id() {
    assert_eq!(color_hex("coral"), ("#ff6b6b", "#3a1f1f"));
}

#[test]
fn color_hex_falls_back_to_slate_for_an_unknown_id() {
    assert_eq!(color_hex("cor-que-nao-existe"), color_hex("slate"));
}

#[test]
fn avatar_letter_uppercases_the_first_character() {
    assert_eq!(avatar_letter("ana"), "A");
    assert_eq!(avatar_letter("  bia"), "B");
}

#[test]
fn avatar_letter_falls_back_to_question_mark_for_empty_nick() {
    assert_eq!(avatar_letter("   "), "?");
}

#[test]
fn default_color_is_a_valid_palette_id() {
    assert!(palette_ids().any(|id| id == DEFAULT_COLOR));
}
