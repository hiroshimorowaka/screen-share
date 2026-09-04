//! WebRTC + screen-capture mechanics, split by responsibility:
//!
//! - [`peer`] — `RtcPeerConnection` construction and the audio-m-line
//!   reservation.
//! - [`connection`] — offer / answer / ICE for one connection.
//! - [`screen_share`] — `getDisplayMedia` capture and its support probe.
//!
//! The Electron desktop-shell bridges (tray notifications, system-audio
//! loopback) live one level up in [`crate::client::desktop_bridge`].
//!
//! This module re-exports the split pieces so call sites keep naming
//! `crate::client::webrtc::{create_offer, capture_display, …}`.

mod connection;
mod peer;
mod screen_share;

pub use connection::{accept_answer, add_ice_candidate, create_answer, create_offer};
pub use peer::{new_peer_connection, reserve_audio_mline};
pub use screen_share::{capture_display, is_display_media_supported};
