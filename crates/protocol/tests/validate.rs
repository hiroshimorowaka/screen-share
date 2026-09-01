use screen_share_protocol::validate::{
    clean_nick, clean_room_name, is_valid_color, NameError, DEFAULT_COLOR, MAX_NICK_LEN,
    MAX_ROOM_NAME_LEN, PALETTE_IDS,
};

#[test]
fn clean_nick_trims_and_accepts_a_normal_name() {
    assert_eq!(clean_nick("  Ana  ").unwrap(), "Ana");
    assert_eq!(clean_nick("Zé do Café 🙂").unwrap(), "Zé do Café 🙂");
}

#[test]
fn clean_nick_rejects_empty_and_whitespace_only() {
    assert_eq!(clean_nick(""), Err(NameError::Empty));
    assert_eq!(clean_nick("   \t "), Err(NameError::Empty));
}

#[test]
fn clean_nick_rejects_something_longer_than_the_limit() {
    let ok = "a".repeat(MAX_NICK_LEN);
    assert_eq!(clean_nick(&ok).unwrap(), ok);
    assert_eq!(
        clean_nick(&"a".repeat(MAX_NICK_LEN + 1)),
        Err(NameError::TooLong)
    );
}

#[test]
fn clean_name_length_counts_characters_not_bytes() {
    // 'é' is two UTF-8 bytes but one character.
    let name = "é".repeat(MAX_ROOM_NAME_LEN);
    assert_eq!(clean_room_name(&name).unwrap(), name);
}

#[test]
fn clean_nick_rejects_control_and_bidi_and_zero_width_characters() {
    for bad in [
        "line\nbreak",
        "tab\there",
        "null\u{0}byte",
        "spoof\u{202E}drowssap", // RIGHT-TO-LEFT OVERRIDE
        "isolate\u{2066}me",     // LEFT-TO-RIGHT ISOLATE
        "zero\u{200B}width",     // ZERO WIDTH SPACE
        "mark\u{200F}",          // RIGHT-TO-LEFT MARK
        "\u{FEFF}bom",           // BYTE ORDER MARK
    ] {
        assert_eq!(
            clean_nick(bad),
            Err(NameError::DisallowedCharacter),
            "should reject {bad:?}"
        );
    }
}

#[test]
fn color_allowlist_is_exactly_the_palette_and_the_default_is_in_it() {
    assert!(is_valid_color("coral"));
    assert!(is_valid_color("slate"));
    assert!(!is_valid_color("rebeccapurple"));
    assert!(!is_valid_color(""));
    assert!(!is_valid_color("#ff0000"));
    assert!(PALETTE_IDS.contains(&DEFAULT_COLOR));
}
