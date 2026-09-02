//! Browser (`wasm32`) tests for `video_mode::apply_video_mode` — the
//! `degradationPreference` / `contentHint` mechanics. Split out so
//! `.cargo/mutants.toml`'s `**/*_wasm_tests.rs` exclusion covers it.

use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_test::*;

use super::*;

wasm_bindgen_test_configure!(run_in_browser);

async fn pc_with_video_track() -> web_sys::RtcPeerConnection {
    let pc = crate::infra::webrtc::new_peer_connection(None).unwrap();
    let generator = web_sys::MediaStreamTrackGenerator::new(
        &web_sys::MediaStreamTrackGeneratorInit::new("video"),
    )
    .unwrap();
    let track: web_sys::MediaStreamTrack = generator.unchecked_into();
    let stream = web_sys::MediaStream::new().unwrap();
    stream.add_track(&track);
    pc.add_track_0(&track, &stream);
    pc
}

fn video_sender(pc: &web_sys::RtcPeerConnection) -> web_sys::RtcRtpSender {
    pc.get_senders().get(0).unchecked_into()
}

fn degradation_of(sender: &web_sys::RtcRtpSender) -> Option<String> {
    js_sys::Reflect::get(
        &sender.get_parameters(),
        &JsValue::from_str("degradationPreference"),
    )
    .unwrap()
    .as_string()
}

fn content_hint_of(sender: &web_sys::RtcRtpSender) -> Option<String> {
    js_sys::Reflect::get(&sender.track().unwrap(), &JsValue::from_str("contentHint"))
        .unwrap()
        .as_string()
}

#[wasm_bindgen_test]
async fn detail_mode_pins_maintain_resolution_and_the_detail_hint() {
    let pc = pc_with_video_track().await;
    apply_video_mode(&pc, VideoMode::Detail).await.unwrap();

    let sender = video_sender(&pc);
    assert_eq!(content_hint_of(&sender).as_deref(), Some("detail"));
    assert_eq!(
        degradation_of(&sender).as_deref(),
        Some("maintain-resolution")
    );
}

#[wasm_bindgen_test]
async fn motion_mode_pins_maintain_framerate_and_the_motion_hint() {
    let pc = pc_with_video_track().await;
    apply_video_mode(&pc, VideoMode::Motion).await.unwrap();

    let sender = video_sender(&pc);
    assert_eq!(content_hint_of(&sender).as_deref(), Some("motion"));
    assert_eq!(
        degradation_of(&sender).as_deref(),
        Some("maintain-framerate")
    );
}

#[wasm_bindgen_test]
async fn switching_modes_live_updates_both_the_hint_and_the_degradation_preference() {
    let pc = pc_with_video_track().await;
    apply_video_mode(&pc, VideoMode::Detail).await.unwrap();
    apply_video_mode(&pc, VideoMode::Motion).await.unwrap();

    let sender = video_sender(&pc);
    assert_eq!(content_hint_of(&sender).as_deref(), Some("motion"));
    assert_eq!(
        degradation_of(&sender).as_deref(),
        Some("maintain-framerate")
    );
}

#[wasm_bindgen_test]
async fn apply_video_mode_is_a_noop_without_a_video_sender() {
    let pc = crate::infra::webrtc::new_peer_connection(None).unwrap();
    apply_video_mode(&pc, VideoMode::Motion).await.unwrap();
}
