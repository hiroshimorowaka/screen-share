//! Browser (`wasm32`) tests for `webrtc::peer` — `RtcPeerConnection`
//! construction.

use wasm_bindgen_test::*;

use super::*;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
fn new_peer_connection_accepts_optional_turn_credentials() {
    assert!(new_peer_connection(None).is_ok());

    let turn = screen_share_protocol::TurnCredentials {
        urls: vec!["turn:relay.example:3478".to_string()],
        username: "user".to_string(),
        password: "pass".to_string(),
    };
    assert!(new_peer_connection(Some(&turn)).is_ok());
}
