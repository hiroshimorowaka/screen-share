//! Browser (`wasm32`) tests for `webrtc::connection` — the offer/answer
//! plumbing and the SDP bitrate-hint round trip.

use wasm_bindgen::JsCast;
use wasm_bindgen_test::*;

use super::*;
use crate::client::webrtc::new_peer_connection;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
async fn create_offer_produces_a_session_description() {
    let pc = new_peer_connection(None).unwrap();
    let sdp = create_offer(&pc).await.unwrap();

    assert!(
        sdp.starts_with("v=0"),
        "an SDP offer starts with a version line, got: {:.40}",
        sdp
    );
}

#[wasm_bindgen_test]
async fn offer_answer_roundtrip_completes_between_two_local_peers() {
    let caller = new_peer_connection(None).unwrap();
    let callee = new_peer_connection(None).unwrap();

    let offer = create_offer(&caller).await.unwrap();
    let answer = create_answer(&callee, &offer).await.unwrap();
    assert!(answer.starts_with("v=0"));

    // Completes without error — `set_remote_description` rejects a
    // malformed or out-of-state answer.
    accept_answer(&caller, &answer).await.unwrap();
}

#[wasm_bindgen_test]
async fn accept_answer_lands_the_start_bitrate_hint_on_the_sharers_remote_description() {
    let sharer = new_peer_connection(None).unwrap();
    let viewer = new_peer_connection(None).unwrap();

    // A real outbound video track, so the negotiated SDP carries a video
    // m-section for the hint to attach to — the generator trick the
    // `quality` wasm tests also use.
    let generator = web_sys::MediaStreamTrackGenerator::new(
        &web_sys::MediaStreamTrackGeneratorInit::new("video"),
    )
    .unwrap();
    let track: web_sys::MediaStreamTrack = generator.unchecked_into();
    let stream = web_sys::MediaStream::new().unwrap();
    stream.add_track(&track);
    sharer.add_track_0(&track, &stream);

    let offer = create_offer(&sharer).await.unwrap();
    let answer = create_answer(&viewer, &offer).await.unwrap();
    accept_answer(&sharer, &answer).await.unwrap();

    // Chrome drops `x-google-*` from the answer it generates; `accept_answer`
    // must put it back, because this remote description is what the sharer's
    // own encoder reads its start bitrate from.
    let remote = sharer
        .remote_description()
        .expect("remote description is set after accept_answer")
        .sdp();
    assert!(
        remote.contains(&format!(
            "x-google-start-bitrate={}",
            screen_share_domain::sdp::VIDEO_START_BITRATE_KBPS
        )),
        "start-bitrate hint missing from the sharer's remote SDP: {remote}"
    );
}
