//! `PeerLink` — the last of step 8's three seam traits: one
//! `RtcPeerConnection`'s negotiation lifecycle (offer / answer / ICE),
//! named as a domain contract instead of four free functions imported
//! wherever a peer connection is negotiated.
//!
//! Unlike `SignalingTransport` and `DisplayCapture`, this one is **not**
//! wired up for fakes — see the structure-refactor progress doc for why.
//! In short: `session::handler`'s negotiation functions don't just call
//! offer/answer/close, they also wire `RtcPeerConnection` event
//! listeners (`ontrack`, `onicecandidate`, `oniceconnectionstatechange`)
//! and store the connection itself in `RoomSession::{outgoing,incoming}`
//! (`HashMap<String, RtcPeerConnection>`, a concrete type shared with
//! `session::{media,quality}`, which need the *rest* of
//! `RtcPeerConnection`'s surface — senders, transceivers — that a
//! narrow offer/answer/ICE trait doesn't cover). Genericizing those
//! functions over `PeerLink` alone wouldn't compile without also
//! abstracting that other surface, and abstracting *all* of it is the
//! `RoomSession` method-API rewrite already scoped and deferred in
//! `docs/superpowers/plans/2026-08-28-refactor-phase-5-roomsession.md`.
//!
//! What this still earns its keep on: `session::handler`'s negotiation
//! functions (`answer_offer`, `accept_answer_from`, `route_ice_candidate`)
//! call `pc.offer()` / `pc.answer(&sdp)` / `pc.accept_answer(&sdp)` /
//! `pc.add_ice_candidate(...)` instead of importing
//! `infra::webrtc::{create_offer, create_answer, accept_answer,
//! add_ice_candidate}` — the negotiation contract reads as one named
//! interface at its call sites, per CLAUDE.md's "design around domain
//! concepts rather than leaking implementation details" API guideline,
//! even though (per above) it isn't yet a fakeable one.

use wasm_bindgen::JsValue;
use web_sys::RtcPeerConnection;

pub(crate) trait PeerLink {
    /// Creates and sets an SDP offer as the local description, returning
    /// the (Opus/bitrate-tuned) SDP to send to the peer.
    async fn offer(&self) -> Result<String, JsValue>;
    /// Applies a received offer as the remote description, then creates
    /// and sets an SDP answer as the local one, returning the SDP to
    /// send back.
    async fn answer(&self, offer_sdp: &str) -> Result<String, JsValue>;
    /// Applies a received answer as the remote description — the
    /// offerer's side of accepting a negotiated connection.
    async fn accept_answer(&self, answer_sdp: &str) -> Result<(), JsValue>;
    /// Adds a remote ICE candidate relayed by the signaling server.
    fn add_ice_candidate(
        &self,
        candidate: &str,
        sdp_mid: Option<String>,
        sdp_m_line_index: Option<u16>,
    );
}

impl PeerLink for RtcPeerConnection {
    async fn offer(&self) -> Result<String, JsValue> {
        crate::infra::webrtc::create_offer(self).await
    }

    async fn answer(&self, offer_sdp: &str) -> Result<String, JsValue> {
        crate::infra::webrtc::create_answer(self, offer_sdp).await
    }

    async fn accept_answer(&self, answer_sdp: &str) -> Result<(), JsValue> {
        crate::infra::webrtc::accept_answer(self, answer_sdp).await
    }

    fn add_ice_candidate(
        &self,
        candidate: &str,
        sdp_mid: Option<String>,
        sdp_m_line_index: Option<u16>,
    ) {
        crate::infra::webrtc::add_ice_candidate(self, candidate, sdp_mid, sdp_m_line_index);
    }
}
