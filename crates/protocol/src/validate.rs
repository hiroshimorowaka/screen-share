//! Shared validation for the user-controlled strings that ride the
//! signaling protocol — a nick, a room name, a colour id. The relay is the
//! source of truth and enforces these; the web client mirrors them for
//! immediate feedback.
//!
//! Why: an unbounded nick is stored and rebroadcast to every member
//! (broadcast amplification), and control / bidirectional formatting
//! characters in a nick let one member visually impersonate another or the
//! "you" label. Length is capped and those characters are rejected. Full
//! Unicode NFC normalisation is deliberately not done here — it would need
//! a dependency, and `crates/protocol` stays serde-only; rejecting the
//! formatting characters is enough to close the spoofing vectors.

/// Longest accepted nick, in Unicode scalar values. Enough for any real
/// display name; short enough that the per-`PeerJoined` rebroadcast can't
/// be turned into an amplifier.
pub const MAX_NICK_LEN: usize = 32;

/// Longest accepted room name, in Unicode scalar values.
pub const MAX_ROOM_NAME_LEN: usize = 64;

/// Most combining marks allowed to stack on one base character. The length
/// cap counts scalar values, so a 32-code-point "Zalgo" nick — dozens of
/// combining marks piled on a couple of bases — passes it while still
/// overflowing its card and bleeding into neighbours. Real text never
/// stacks this many: even fully-decomposed Vietnamese tops out at two (a
/// vowel modifier plus a tone).
pub const MAX_MARKS_PER_CLUSTER: usize = 4;

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
    /// More than [`MAX_MARKS_PER_CLUSTER`] combining marks stacked on one
    /// base character ("Zalgo" text) — passes the length cap but breaks
    /// the layout.
    ExcessiveCombiningMarks,
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

/// `true` for the combining marks a "Zalgo" generator stacks: the
/// Combining Diacritical Marks block and its Extended / Supplement / for
/// Symbols / Half Marks siblings, plus the Cyrillic combining range. Not a
/// full Unicode `Mn`/`Mc`/`Me` test (that needs a table / a dependency) —
/// just the blocks the abuse actually draws from.
fn is_combining_mark(c: char) -> bool {
    matches!(c,
        '\u{0300}'..='\u{036F}'  // Combining Diacritical Marks
        | '\u{0483}'..='\u{0489}'  // Cyrillic combining
        | '\u{1AB0}'..='\u{1AFF}'  // Combining Diacritical Marks Extended
        | '\u{1DC0}'..='\u{1DFF}'  // Combining Diacritical Marks Supplement
        | '\u{20D0}'..='\u{20FF}'  // Combining Diacritical Marks for Symbols
        | '\u{FE20}'..='\u{FE2F}'  // Combining Half Marks
    )
}

/// Whether any run of consecutive combining marks in `s` is longer than
/// [`MAX_MARKS_PER_CLUSTER`].
fn has_excessive_mark_stacking(s: &str) -> bool {
    let mut run = 0usize;
    for c in s.chars() {
        if is_combining_mark(c) {
            run += 1;
            if run > MAX_MARKS_PER_CLUSTER {
                return true;
            }
        } else {
            run = 0;
        }
    }
    false
}

/// Trims `input`, then accepts it as a display name of at most `max_len`
/// characters containing no disallowed character and no runaway stack of
/// combining marks.
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
    if has_excessive_mark_stacking(trimmed) {
        return Err(NameError::ExcessiveCombiningMarks);
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
