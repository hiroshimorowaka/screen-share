use screen_share_protocol::validate::{
    clean_nick, clean_room_name, is_valid_color, NameError, DEFAULT_COLOR, MAX_MARKS_PER_CLUSTER,
    MAX_NICK_LEN, MAX_ROOM_NAME_LEN, PALETTE_IDS,
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
fn clean_nick_rejects_zalgo_style_mark_stacking() {
    // A base char with many combining diacritical marks piled on it: few
    // enough scalar values to pass the length cap, but it overflows a card.
    let zalgo = format!("A{}", "\u{0301}".repeat(MAX_MARKS_PER_CLUSTER + 1));
    assert_eq!(clean_nick(&zalgo), Err(NameError::ExcessiveCombiningMarks));

    // A longer run further into the string, not just at the start.
    let heavy = format!("test{}", "\u{036F}".repeat(20));
    assert_eq!(clean_nick(&heavy), Err(NameError::ExcessiveCombiningMarks));
}

#[test]
fn clean_nick_allows_a_few_combining_marks_as_real_decomposed_text_would_have() {
    // Vietnamese "Nguyễn" fully decomposed: base vowel + circumflex + tone
    // — two marks on one cluster, well under the cap.
    let decomposed = "Nguye\u{0302}\u{0303}n Tha\u{0301}i";
    assert_eq!(clean_nick(decomposed).unwrap(), decomposed);

    // Exactly at the cap on one cluster is still fine.
    let at_cap = format!("o{}", "\u{0308}".repeat(MAX_MARKS_PER_CLUSTER));
    assert_eq!(clean_nick(&at_cap).unwrap(), at_cap);

    // Many single marks spread over many bases: the run counter must reset
    // at each base, so the whole-string total is irrelevant.
    let spread = "a\u{0301}e\u{0301}i\u{0301}o\u{0301}u\u{0301}y\u{0301}";
    assert_eq!(clean_nick(spread).unwrap(), spread);
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
