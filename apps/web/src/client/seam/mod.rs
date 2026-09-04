//! The browser-capability seams: one trait per browser boundary the
//! `session`/`room` runtime depends on, with the real `web-sys` impls
//! here and a fake substitutable in tests.
//!
//! - [`signaling_transport`] — typed send/close over the signaling socket
//!   (`WsClient`).
//! - [`display_capture`] — `navigator.mediaDevices.getDisplayMedia`.
//! - [`peer_link`] — one `RtcPeerConnection`'s offer/answer/ICE lifecycle.

pub(crate) mod display_capture;
pub(crate) mod peer_link;
pub(crate) mod signaling_transport;
