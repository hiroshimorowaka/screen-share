//! Whether this member currently has an outgoing screen share running
//! and, if so, the captured stream powering it — one enum instead of a
//! bare `Option<MediaStream>`, so "are we sharing" and "which stream"
//! can never disagree.
//!
//! Deliberately not a reactive signal: `RoomSession::sharing` is read
//! and written from JS event callbacks and from teardown code that must
//! stay callable with no Leptos reactive runtime in scope (see
//! `media::teardown_local_share` and `media/wasm_tests.rs`). The
//! reactive `is_sharing` signal the UI reads (`session::state::RoomState`)
//! stays a separate value, kept in sync at the same points it always
//! was — that pairing is the reactive/imperative boundary, not
//! something this enum collapses.

#[cfg(feature = "hydrate")]
#[derive(Default)]
pub(crate) enum SharingState {
    #[default]
    Idle,
    Sharing {
        stream: web_sys::MediaStream,
    },
}

#[cfg(feature = "hydrate")]
impl SharingState {
    pub(crate) fn is_sharing(&self) -> bool {
        matches!(self, Self::Sharing { .. })
    }

    pub(crate) fn stream(&self) -> Option<&web_sys::MediaStream> {
        match self {
            Self::Idle => None,
            Self::Sharing { stream } => Some(stream),
        }
    }

    /// Takes the stream out, leaving `Idle` behind — the enum equivalent
    /// of `Option::take`.
    pub(crate) fn take(&mut self) -> Option<web_sys::MediaStream> {
        match std::mem::replace(self, Self::Idle) {
            Self::Idle => None,
            Self::Sharing { stream } => Some(stream),
        }
    }
}

#[cfg(all(test, target_arch = "wasm32", feature = "hydrate"))]
#[path = "sharing_state/wasm_tests.rs"]
mod wasm_tests;
