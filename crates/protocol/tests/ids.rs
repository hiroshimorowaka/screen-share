//! Typed wire identifiers: construction bounds and the bare-string serde
//! representation.

use screen_share_protocol::ids::{Color, IdError, Nick, PeerId, RoomCode};

#[test]
fn peer_id_round_trips_through_str() {
    let id = PeerId::parse("abc-123").unwrap();
    assert_eq!(id.as_str(), "abc-123");
    assert_eq!(id.to_string(), "abc-123");
    assert_eq!("abc-123".parse::<PeerId>().unwrap(), id);
}

#[test]
fn peer_id_rejects_empty_and_overlong() {
    assert_eq!(PeerId::parse(""), Err(IdError::PeerId));
    assert_eq!(PeerId::parse("x".repeat(65)), Err(IdError::PeerId));
    // The cap itself is inclusive: exactly 64 bytes is still accepted.
    assert!(PeerId::parse("x".repeat(64)).is_ok());
    // A UUID and an 8-char room-style code are both comfortably inside.
    assert!(PeerId::parse("550e8400-e29b-41d4-a716-446655440000").is_ok());
}

#[test]
fn room_code_accepts_any_plausible_lookup_key_and_rejects_the_bogus() {
    // Lenient on purpose: the relay's own generator emits 8 chars of a
    // fixed alphabet, but a stricter rule would turn a "room not found"
    // into a "rejected" for an unknown code (a behaviour change).
    assert!(RoomCode::parse("ABCD1234").is_ok());
    assert!(RoomCode::parse("abcd1234").is_ok());
    assert_eq!(RoomCode::parse(""), Err(IdError::RoomCode));
    assert_eq!(RoomCode::parse("x".repeat(65)), Err(IdError::RoomCode));
    // The cap itself is inclusive: exactly 64 bytes is still accepted.
    assert!(RoomCode::parse("x".repeat(64)).is_ok());
}

#[test]
fn nick_trims_and_matches_the_shared_validator() {
    assert_eq!(Nick::parse("  Ana ").unwrap().as_str(), "Ana");
    assert_eq!(Nick::parse("   "), Err(IdError::Nick));
    assert_eq!(Nick::parse("n".repeat(33)), Err(IdError::Nick));
    // A bidi override is a spoofing vector `validate::clean_nick` rejects.
    assert_eq!(Nick::parse("a\u{202E}b"), Err(IdError::Nick));
}

#[test]
fn color_accepts_only_palette_ids() {
    assert!(Color::parse("coral").is_ok());
    assert_eq!(Color::parse("#1a2b3c"), Err(IdError::Color));
    assert_eq!(Color::parse("chartreuse"), Err(IdError::Color));
}

#[test]
fn serde_representation_is_a_bare_string() {
    let id = PeerId::parse("p1").unwrap();
    assert_eq!(serde_json::to_string(&id).unwrap(), "\"p1\"");

    let back: PeerId = serde_json::from_str("\"p1\"").unwrap();
    assert_eq!(back, id);

    assert!(serde_json::from_str::<PeerId>("\"\"").is_err());
    assert!(serde_json::from_str::<Color>("\"not-a-colour\"").is_err());
}

#[test]
fn nick_deserialisation_applies_the_same_trim_as_parse() {
    let n: Nick = serde_json::from_str("\"  Bia  \"").unwrap();
    assert_eq!(n.as_str(), "Bia");
}
