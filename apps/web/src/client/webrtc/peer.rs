//! `RtcPeerConnection` construction: the ICE-server config (STUN, plus
//! TURN when the deployment has one) and the up-front audio-m-line
//! reservation for a share that starts video-only.

use wasm_bindgen::prelude::*;
use web_sys::{MediaStream, RtcConfiguration, RtcIceServer, RtcPeerConnection};

/// A public STUN server, used only so each peer can discover its own
/// public-facing address for the ICE candidates it offers — no media or
/// signaling data ever passes through it.
const STUN_SERVER_URL: &str = "stun:stun.l.google.com:19302";

/// `turn` is `None` on a deployment with no TURN server configured (or
/// briefly, before the join snapshot carrying it has arrived) — the
/// connection still works for peers that don't need a relay, just without
/// a fallback for the ones that do.
pub fn new_peer_connection(
    turn: Option<&screen_share_protocol::TurnCredentials>,
) -> Result<RtcPeerConnection, JsValue> {
    let stun_server = RtcIceServer::new();
    let stun_urls = js_sys::Array::new();
    stun_urls.push(&JsValue::from_str(STUN_SERVER_URL));
    stun_server.set_urls(&JsValue::from(stun_urls));

    let servers = js_sys::Array::new();
    servers.push(&stun_server);

    if let Some(turn) = turn {
        let turn_server = RtcIceServer::new();
        let turn_urls = js_sys::Array::new();
        for url in &turn.urls {
            turn_urls.push(&JsValue::from_str(url));
        }
        turn_server.set_urls(&JsValue::from(turn_urls));
        turn_server.set_username(&turn.username);
        turn_server.set_credential(&turn.password);
        servers.push(&turn_server);
    }

    let config = RtcConfiguration::new();
    config.set_ice_servers(&JsValue::from(servers));

    RtcPeerConnection::new_with_configuration(&config)
}

/// Negotiates a `sendonly` audio m-line on `pc` up front, with no track,
/// tied to `stream` (the share's video stream).
///
/// Used when a share starts without audio. A later "trocar fonte" can add
/// audio (a shared tab with sound, or the desktop system-audio loopback),
/// and `session::media::replace_outgoing_tracks` swaps it in with
/// `RTCRtpSender.replaceTrack` — which needs no renegotiation, but only
/// reaches an m-line that already existed when the connection was
/// answered. This signaling path never re-offers an open viewer
/// connection, so without this reservation a viewer already watching a
/// silent share would stay silent after the switch until they re-watched.
/// Binding it to `stream` makes the viewer group the eventual audio track
/// with the video it is already playing, so `ontrack` doesn't need a
/// second stream to attach.
pub fn reserve_audio_mline(pc: &RtcPeerConnection, stream: &MediaStream) {
    let streams = js_sys::Array::new();
    streams.push(stream);
    let init = web_sys::RtcRtpTransceiverInit::new();
    init.set_direction(web_sys::RtcRtpTransceiverDirection::Sendonly);
    init.set_streams(&streams);
    pc.add_transceiver_with_str_and_init("audio", &init);
}

#[cfg(all(test, target_arch = "wasm32", feature = "hydrate"))]
#[path = "peer_wasm_tests.rs"]
mod wasm_tests;
