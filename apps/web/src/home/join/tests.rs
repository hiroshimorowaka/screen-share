//! Unit tests for `join_room`, split out of src/join_room.rs to keep it
//! readable (refactor Phase 4). Kept in-crate: they exercise items that
//! are not part of the crate's public API.

use super::*;

#[test]
fn extract_room_code_accepts_a_bare_code() {
    assert_eq!(extract_room_code("ab3d9f2k"), Some("AB3D9F2K".to_string()));
}

#[test]
fn extract_room_code_accepts_a_full_link() {
    assert_eq!(
        extract_room_code("https://example.com/r/AB3D9F2K"),
        Some("AB3D9F2K".to_string())
    );
}

#[test]
fn extract_room_code_strips_trailing_slash_and_query_string() {
    assert_eq!(
        extract_room_code("https://example.com/r/AB3D9F2K/?x=1"),
        Some("AB3D9F2K".to_string())
    );
}

#[test]
fn extract_room_code_trims_surrounding_whitespace() {
    assert_eq!(
        extract_room_code("  AB3D9F2K  "),
        Some("AB3D9F2K".to_string())
    );
}

#[test]
fn extract_room_code_rejects_blank_input() {
    assert_eq!(extract_room_code("   "), None);
}
