//! Small `Copy` state structs that bundle related reactive signals so a
//! consumer takes one value instead of threading several loose ones by
//! hand. Each groups signals that are only ever meaningful, and read or
//! written, together — see `RoomPage`'s remaining ~450-line body and the
//! structure-refactor progress notes for why these three exist.

use std::collections::{HashMap, HashSet};

use leptos::prelude::*;
use screen_share_protocol::QualityLevel;

/// The reactive state around this member's own outgoing share: whether
/// it's live, the self-preview visibility, and the audio-quality /
/// self-test signals layered on top of it. Every field is a cheap
/// `Copy` signal handle, so passing the whole struct costs no more than
/// passing one of its fields.
#[cfg_attr(not(feature = "hydrate"), allow(dead_code))]
#[derive(Clone, Copy)]
pub(crate) struct ShareUi {
    pub(crate) is_sharing: ReadSignal<bool>,
    pub(crate) set_is_sharing: WriteSignal<bool>,
    pub(crate) own_preview_hidden: RwSignal<bool>,
    /// Whether the sharer has silenced their own outgoing audio (the
    /// track stays published, viewers just hear silence). Reset to
    /// `false` when a share ends.
    pub(crate) audio_muted: RwSignal<bool>,
    /// Whether the current share's captured stream actually carries an
    /// audio track. The web side never learns whether the sharer ticked
    /// "compartilhar áudio" in the desktop picker, so this is the
    /// closest signal for "this share has audio". Reset when sharing
    /// stops.
    pub(crate) share_has_audio: RwSignal<bool>,
    /// Set by the audio self-test once a share of ours has been probed;
    /// `None` means "nothing wrong / not checked yet".
    pub(crate) audio_warning: RwSignal<Option<&'static str>>,
    /// Bumped on every source switch so effects keyed to `is_sharing`
    /// alone (which stays `true` across a switch) re-run against the
    /// new stream.
    pub(crate) share_generation: RwSignal<u32>,
}

/// Per-peer live media state a member's card reads: this viewer's chosen
/// volume/mute for that peer, and the negotiated quality tier.
#[derive(Clone, Copy)]
pub(crate) struct PeerMedia {
    pub(crate) volume_by_peer: RwSignal<HashMap<String, f64>>,
    pub(crate) muted_by_peer: RwSignal<HashSet<String>>,
    pub(crate) quality_by_peer: RwSignal<HashMap<String, QualityLevel>>,
}
