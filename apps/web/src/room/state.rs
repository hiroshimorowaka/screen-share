//! `RoomState` — every reactive signal the authenticated room view and
//! its runtime share, in one `Copy` struct created by [`RoomState::new`]
//! and delivered once via `provide_context` instead of the four
//! hand-threaded bundles it replaces (`RoomState`, `ShareUi`,
//! `PeerMedia`, `MemberCardSignals`).
//!
//! Every field is a signal handle (itself `Copy`), so passing the whole
//! struct costs no more than passing one field.

use std::collections::{HashMap, HashSet};

use leptos::prelude::*;
use screen_share_protocol::{QualityLevel, TurnCredentials};

use crate::room::audio::AudioPreset;
use crate::room::audio_health::AudioHealth;
use crate::room::video_mode::VideoMode;
use crate::room::RoomMember;

/// The gate's status sentence before a member is authenticated. Also what
/// a dismissible pre-auth error on `status` reverts to — see
/// `crate::client::dom::auto_dismiss_error` in `room::page`.
pub(crate) const INITIAL_STATUS: &str = "Informe o nick da sala.";

/// `ssr` builds only ever pass this through inert stub functions, so an
/// `ssr`-only compile sees no reads and would flag it as dead code.
#[cfg_attr(not(feature = "hydrate"), allow(dead_code))]
#[derive(Clone, Copy)]
pub(crate) struct RoomState {
    // --- roster ---
    pub(crate) members: ReadSignal<Vec<RoomMember>>,
    pub(crate) set_members: WriteSignal<Vec<RoomMember>>,
    pub(crate) my_peer_id: ReadSignal<Option<String>>,
    pub(crate) set_my_peer_id: WriteSignal<Option<String>>,

    // --- connection lifecycle ---
    pub(crate) status: ReadSignal<String>,
    pub(crate) set_status: WriteSignal<String>,
    pub(crate) authenticated: ReadSignal<bool>,
    pub(crate) set_authenticated: WriteSignal<bool>,
    pub(crate) room_exists: ReadSignal<Option<bool>>,
    pub(crate) set_room_exists: WriteSignal<Option<bool>>,
    pub(crate) room_name: ReadSignal<Option<String>>,
    pub(crate) set_room_name: WriteSignal<Option<String>>,
    pub(crate) requires_password: ReadSignal<bool>,
    pub(crate) set_requires_password: WriteSignal<bool>,
    /// Set once from the join snapshot, then reused for every peer
    /// connection this WebSocket session opens. `None` on a deployment
    /// with no TURN server configured.
    pub(crate) turn_credentials: RwSignal<Option<TurnCredentials>>,

    // --- watch graph ---
    pub(crate) watching: RwSignal<HashSet<String>>,
    pub(crate) watchers_by_sharer: RwSignal<HashMap<String, Vec<String>>>,
    pub(crate) expanded: RwSignal<Option<String>>,

    // --- this member's own share (was `ShareUi`) ---
    pub(crate) is_sharing: ReadSignal<bool>,
    pub(crate) set_is_sharing: WriteSignal<bool>,
    pub(crate) own_preview_hidden: RwSignal<bool>,
    /// The sharer silenced their own outgoing audio (the track stays
    /// published; viewers hear silence). Reset when a share ends.
    pub(crate) audio_muted: RwSignal<bool>,
    /// Whether the current share's captured stream actually carries an
    /// audio track. Reset when sharing stops.
    pub(crate) share_has_audio: RwSignal<bool>,
    /// The audio self-test's verdict once a share of ours is probed.
    /// Carries the full [`AudioHealth`] (not just its message) so the chip
    /// can tell a hard failure (no track at all — red) from a soft one (a
    /// track that just stayed silent so far — amber, might start any
    /// moment) apart, instead of collapsing both into one warning string.
    pub(crate) audio_health: RwSignal<AudioHealth>,
    /// Bumped on every source switch so effects keyed to `is_sharing`
    /// alone (which stays `true` across a switch) re-run.
    pub(crate) share_generation: RwSignal<u32>,
    /// The sharer's chosen outgoing audio quality — read when opening a
    /// new viewer connection.
    pub(crate) audio_preset: RwSignal<AudioPreset>,
    /// The sharer's chosen video mode (protect detail vs. motion).
    pub(crate) video_mode: RwSignal<VideoMode>,

    // --- per-peer diagnostics a card reads (was `PeerMedia` + more) ---
    pub(crate) latency_by_peer: RwSignal<HashMap<String, u32>>,
    pub(crate) connection_errors: RwSignal<HashSet<String>>,
    pub(crate) quality_by_peer: RwSignal<HashMap<String, QualityLevel>>,
    pub(crate) volume_by_peer: RwSignal<HashMap<String, f64>>,
    pub(crate) muted_by_peer: RwSignal<HashSet<String>>,

    // --- view chrome shared by the control bar and the cards ---
    pub(crate) hide_idle: RwSignal<bool>,
    pub(crate) controls_visible: RwSignal<bool>,
    pub(crate) is_touch: ReadSignal<bool>,
    pub(crate) set_is_touch: WriteSignal<bool>,
    pub(crate) invite_copied: RwSignal<bool>,
}

impl RoomState {
    /// Creates every signal the room needs. Call once in the room page,
    /// then `provide_context` the result.
    #[must_use]
    pub(crate) fn new() -> Self {
        let (members, set_members) = signal(Vec::<RoomMember>::new());
        let (my_peer_id, set_my_peer_id) = signal(None::<String>);
        let (is_sharing, set_is_sharing) = signal(false);
        let (status, set_status) = signal(INITIAL_STATUS.to_string());
        let (authenticated, set_authenticated) = signal(false);
        let (room_exists, set_room_exists) = signal(None::<bool>);
        let (room_name, set_room_name) = signal(None::<String>);
        // Assume a password may be required until the room check resolves;
        // the join panel that reads this stays hidden until then anyway.
        let (requires_password, set_requires_password) = signal(true);
        let (is_touch, set_is_touch) = signal(false);

        Self {
            members,
            set_members,
            my_peer_id,
            set_my_peer_id,
            status,
            set_status,
            authenticated,
            set_authenticated,
            room_exists,
            set_room_exists,
            room_name,
            set_room_name,
            requires_password,
            set_requires_password,
            turn_credentials: RwSignal::new(None),
            watching: RwSignal::new(HashSet::new()),
            watchers_by_sharer: RwSignal::new(HashMap::new()),
            expanded: RwSignal::new(None),
            is_sharing,
            set_is_sharing,
            own_preview_hidden: RwSignal::new(false),
            audio_muted: RwSignal::new(false),
            share_has_audio: RwSignal::new(false),
            audio_health: RwSignal::new(AudioHealth::NotShared),
            share_generation: RwSignal::new(0),
            audio_preset: RwSignal::new(AudioPreset::default()),
            video_mode: RwSignal::new(VideoMode::default()),
            latency_by_peer: RwSignal::new(HashMap::new()),
            connection_errors: RwSignal::new(HashSet::new()),
            quality_by_peer: RwSignal::new(HashMap::new()),
            volume_by_peer: RwSignal::new(HashMap::new()),
            muted_by_peer: RwSignal::new(HashSet::new()),
            hide_idle: RwSignal::new(false),
            controls_visible: RwSignal::new(true),
            is_touch,
            set_is_touch,
            invite_copied: RwSignal::new(false),
        }
    }
}

#[cfg(test)]
#[path = "state_tests.rs"]
mod tests;
