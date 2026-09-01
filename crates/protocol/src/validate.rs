//! Shared validation for the user-controlled strings that ride the
//! signaling protocol — a nick, a room name, a colour id. The relay is the
//! source of truth and enforces these; the web client mirrors them for
//! immediate feedback.
//!
//! Purpose (finding F08): a multi-megabyte nick is stored and rebroadcast
//! to every member (broadcast amplification), and control / bidirectional
//! formatting characters in a nick let one member visually impersonate
//! another or the "you" label. Length is capped and those characters are
//! rejected. Full Unicode NFC normalisation is intentionally *not* done
//! here — it would need a dependency, and `crates/protocol` stays
//! serde-only; rejecting the formatting characters removes the spoofing
//! vectors the audit called out.

/// Longest accepted nick, in Unicode scalar values. Enough for any real
/// display name; short enough that the per-`PeerJoined` rebroadcast can't
/// be turned into an amplifier.
pub const MAX_NICK_LEN: usize = 32;

/// Longest accepted room name, in Unicode scalar values.
pub const MAX_ROOM_NAME_LEN: usize = 64;

/// The colour ids a member may pick — the fixed avatar/border palette.
/// Kept here (not just in the web app's render table) so the relay can
/// reject anything else instead of silently falling back to a default.
pub const PALETTE_IDS: &[&str] = &[
    "coral",
    "amber",
    "gold",
    "lime",
    "teal",
    "sky",
    "periwinkle",
    "violet",
    "pink",
    "slate",
];

/// The colour used when none is chosen; always a member of [`PALETTE_IDS`].
pub const DEFAULT_COLOR: &str = "coral";

/// Why a user-supplied string was rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameError {
    /// Empty (or whitespace-only) after trimming.
    Empty,
    /// More than the field's maximum number of characters.
    TooLong,
    /// Contains a control character or a bidirectional / zero-width
    /// formatting character — used for visual spoofing, never in a real
    /// name.
    DisallowedCharacter,
}

/// `true` for characters that must never appear in a display name: C0/C1
/// controls, the bidi overrides/isolates, the LRM/RLM/ALM marks, the
/// zero-width joiners/spaces, and the BOM.
fn is_disallowed(c: char) -> bool {
    c.is_control()
        || matches!(c,
            '\u{061C}'                 // ARABIC LETTER MARK
            | '\u{200B}'..='\u{200F}'  // ZWSP, ZWNJ, ZWJ, LRM, RLM
            | '\u{202A}'..='\u{202E}'  // LRE, RLE, PDF, LRO, RLO
            | '\u{2066}'..='\u{2069}'  // LRI, RLI, FSI, PDI
            | '\u{00AD}'               // SOFT HYPHEN
            | '\u{FEFF}'               // ZERO WIDTH NO-BREAK SPACE / BOM
        )
}

/// Trims `input`, then accepts it as a display name of at most `max_len`
/// characters containing no disallowed character.
///
/// # Errors
///
/// [`NameError`] describing the first rule the input breaks.
pub fn clean_name(input: &str, max_len: usize) -> Result<String, NameError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(NameError::Empty);
    }
    if trimmed.chars().count() > max_len {
        return Err(NameError::TooLong);
    }
    if trimmed.chars().any(is_disallowed) {
        return Err(NameError::DisallowedCharacter);
    }
    Ok(trimmed.to_owned())
}

/// A nick: [`clean_name`] with [`MAX_NICK_LEN`].
pub fn clean_nick(input: &str) -> Result<String, NameError> {
    clean_name(input, MAX_NICK_LEN)
}

/// A room name: [`clean_name`] with [`MAX_ROOM_NAME_LEN`].
pub fn clean_room_name(input: &str) -> Result<String, NameError> {
    clean_name(input, MAX_ROOM_NAME_LEN)
}

/// Whether `id` is one of the allowed palette colours.
pub fn is_valid_color(id: &str) -> bool {
    PALETTE_IDS.contains(&id)
}
