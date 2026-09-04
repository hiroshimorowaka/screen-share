//! The room feature slice: the authenticated room view and everything
//! its runtime needs — the socket lifecycle (`connection`, `reconnect`),
//! the `ServerMessage` dispatch (`messages`), the per-capability handlers
//! (`media`, `audio`, `video_mode`, `quality`, `watch`), the reactive
//! store (`state`), and the view components (`components`).

pub mod audio;
pub mod audio_health;
pub mod components;
pub(crate) mod connection;
pub(crate) mod invite;
pub mod latency;
pub mod media;
pub(crate) mod media_controls;
pub mod messages;
pub mod quality;
pub mod reconnect;
pub(crate) mod room_check;
pub(crate) mod session;
pub(crate) mod share_effects;
pub(crate) mod sharing_state;
pub(crate) mod state;
pub(crate) mod touch;
pub mod video_mode;
pub(crate) mod watch;

#[cfg(debug_assertions)]
pub mod dev_preview;

pub(crate) use connection::{adopt_pending_session, setup_room_connection};
#[cfg(feature = "hydrate")]
pub(crate) use invite::{build_invite_link, copy_invite_link};
pub(crate) use session::RoomSession;
#[cfg(feature = "hydrate")]
pub(crate) use session::{LinkDirection, PeerLink};
#[cfg(feature = "hydrate")]
pub(crate) use sharing_state::SharingState;
pub(crate) use state::RoomState;
// Only read by `share_effects`, which is itself only reachable from
// hydrate-only call sites — an `ssr` build sees no reads.
#[cfg_attr(not(feature = "hydrate"), allow(unused_imports))]
pub(crate) use watch::leave_room;

/// One member of a room, as the roster UI needs it. `sharing` is never
/// `true` on the local member's own card.
#[derive(Clone, PartialEq)]
pub struct RoomMember {
    pub peer_id: String,
    pub nick: String,
    pub color: String,
    pub sharing: bool,
}
