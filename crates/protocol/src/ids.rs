//! Typed identifiers for the signaling wire. Each is a validated `String`
//! newtype: construct via `parse` at the boundary, and every downstream
//! consumer can then trust the value ("parse, don't validate"). The serde
//! representation is a bare string, so the JSON wire format is
//! byte-identical to the pre-newtype protocol.
//!
//! These are adopted on the **server-to-client** messages and the info
//! structs, whose values the relay always generates or has already
//! validated. `ClientMessage` deliberately keeps bare `String` fields: an
//! inbound message that fails a stricter type would change how the relay
//! reacts to it (a malformed `ClientMessage` is silently ignored today,
//! whereas a bad nick in `CreateRoom` draws an explicit
//! `ServerMessage::InvalidInput`) — the relay keeps owning that
//! distinction.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::validate;

/// Why a wire identifier was rejected. One variant per identifier type so
/// a caller can tell which field was wrong and surface the right message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum IdError {
    /// A peer id that is empty or implausibly long.
    #[error("invalid peer id")]
    PeerId,
    /// A room code that is empty or implausibly long.
    #[error("invalid room code")]
    RoomCode,
    /// A nick that fails [`validate::clean_nick`] (empty, too long, or
    /// carrying control / spoofing characters).
    #[error("invalid nick")]
    Nick,
    /// A colour that is not one of [`validate::PALETTE_IDS`].
    #[error("invalid colour")]
    Color,
}

/// Upper bound on the length of an opaque id (peer id, room code), in
/// bytes. Real values are far shorter — a UUID peer id is 36 chars, a
/// room code is 8 — so this rejects only clearly-bogus input while never
/// refusing a value that could name a real peer or room. Kept lenient on
/// purpose: matching the *generator's* exact shape would turn a
/// currently-"not found" lookup into a "rejected" one.
const MAX_OPAQUE_ID_LEN: usize = 64;

/// Shared trait impls for a `String` newtype: `as_str`, `Display`,
/// `FromStr`, and the serde `try_from`/`into` bridge. Each type's
/// `parse` (the actual validation) is defined separately below.
macro_rules! string_newtype {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
        #[serde(try_from = "String", into = "String")]
        pub struct $name(String);

        impl $name {
            /// Borrows the underlying string.
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }

            /// Wraps a value the relay itself minted or has already
            /// validated, skipping the [`Self::parse`] checks. Used at the
            /// signaling boundary where the string is known-good (a
            /// freshly generated id, or a nick/colour that already passed
            /// validation on join) and re-parsing would only be a
            /// tautology. Never call this on unvalidated client input.
            #[must_use]
            pub fn from_relay(value: impl Into<String>) -> Self {
                Self(value.into())
            }
        }

        impl std::borrow::Borrow<str> for $name {
            fn borrow(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl FromStr for $name {
            type Err = IdError;
            fn from_str(s: &str) -> Result<Self, IdError> {
                Self::parse(s)
            }
        }

        impl TryFrom<String> for $name {
            type Error = IdError;
            fn try_from(s: String) -> Result<Self, IdError> {
                Self::parse(s)
            }
        }

        impl From<$name> for String {
            fn from(v: $name) -> String {
                v.0
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                &self.0
            }
        }
    };
}

string_newtype! {
    /// A peer's connection id, minted by the relay (`Uuid::new_v4`).
    PeerId
}
string_newtype! {
    /// A room's short code, minted by the relay.
    RoomCode
}
string_newtype! {
    /// A member's chosen display name, already run through
    /// [`validate::clean_nick`].
    Nick
}
string_newtype! {
    /// A member's chosen avatar/border colour — one of
    /// [`validate::PALETTE_IDS`].
    Color
}

impl PeerId {
    /// Accepts any non-empty string up to [`MAX_OPAQUE_ID_LEN`] bytes.
    ///
    /// # Errors
    /// [`IdError::PeerId`] if empty or over the length cap.
    pub fn parse(raw: impl Into<String>) -> Result<Self, IdError> {
        let s = raw.into();
        if s.is_empty() || s.len() > MAX_OPAQUE_ID_LEN {
            return Err(IdError::PeerId);
        }
        Ok(Self(s))
    }
}

impl RoomCode {
    /// Accepts any non-empty string up to [`MAX_OPAQUE_ID_LEN`] bytes.
    ///
    /// # Errors
    /// [`IdError::RoomCode`] if empty or over the length cap.
    pub fn parse(raw: impl Into<String>) -> Result<Self, IdError> {
        let s = raw.into();
        if s.is_empty() || s.len() > MAX_OPAQUE_ID_LEN {
            return Err(IdError::RoomCode);
        }
        Ok(Self(s))
    }
}

impl Nick {
    /// Trims and validates per [`validate::clean_nick`], storing the
    /// cleaned value.
    ///
    /// # Errors
    /// [`IdError::Nick`] for any [`validate::NameError`].
    pub fn parse(raw: impl Into<String>) -> Result<Self, IdError> {
        let s = raw.into();
        validate::clean_nick(&s)
            .map(Self)
            .map_err(|_| IdError::Nick)
    }
}

impl Color {
    /// Accepts one of [`validate::PALETTE_IDS`].
    ///
    /// # Errors
    /// [`IdError::Color`] for anything else.
    pub fn parse(raw: impl Into<String>) -> Result<Self, IdError> {
        let s = raw.into();
        if validate::is_valid_color(&s) {
            Ok(Self(s))
        } else {
            Err(IdError::Color)
        }
    }
}
