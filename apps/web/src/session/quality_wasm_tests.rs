//! Browser (`wasm32`) tests for the parts of `quality` that touch real
//! WebRTC objects — the encoding-parameter mechanics `apply_tier` relies
//! on. Split into its own file so `.cargo/mutants.toml`'s
//! `**/*_wasm_tests.rs` exclusion covers it. Run with:
//!
//! ```text
//! cargo test -p screen_share --target wasm32-unknown-unknown \
//!   --no-default-features --features hydrate
//! ```

use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_test::*;

use super::*;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
fn configure_encoding_pins_bitrate_scale_and_framerate() {
    let encoding = web_sys::RtcRtpEncodingParameters::new();
    let preset = preset_for(Tier::Medium);

    configure_encoding(&encoding, preset);

    assert_eq!(encoding.get_max_bitrate(), Some(preset.max_bitrate_bps));
    assert_eq!(
        encoding.get_scale_resolution_down_by(),
        Some(preset.scale_down)
    );
    let framerate = js_sys::Reflect::get(&encoding, &JsValue::from_str("maxFramerate"))
        .unwrap()
        .as_f64();
    assert_eq!(framerate, Some(preset.max_framerate));
    // `degradationPreference` is deliberately NOT touched here — it belongs
    // to `session::video_mode` (see its wasm tests).
    let degradation =
        js_sys::Reflect::get(&encoding, &JsValue::from_str("degradationPreference")).unwrap();
    assert!(degradation.is_undefined());
}

#[wasm_bindgen_test]
async fn apply_tier_writes_the_selected_tier_onto_the_video_senders_parameters() {
    let pc = crate::infra::webrtc::new_peer_connection(None).unwrap();
    let generator = web_sys::MediaStreamTrackGenerator::new(
        &web_sys::MediaStreamTrackGeneratorInit::new("video"),
    )
    .unwrap();
    let track: web_sys::MediaStreamTrack = generator.unchecked_into();
    let stream = web_sys::MediaStream::new().unwrap();
    stream.add_track(&track);
    pc.add_track_0(&track, &stream);

    apply_tier(&pc, Tier::Low).await.unwrap();

    let sender: web_sys::RtcRtpSender = pc.get_senders().get(0).unchecked_into();
    let encodings = sender.get_parameters().get_encodings().unwrap();
    let encoding: web_sys::RtcRtpEncodingParameters = encodings.get(0).unchecked_into();
    assert_eq!(
        encoding.get_max_bitrate(),
        Some(preset_for(Tier::Low).max_bitrate_bps)
    );
}

#[wasm_bindgen_test]
async fn apply_tier_is_a_noop_when_no_video_sender_exists_yet() {
    let pc = crate::infra::webrtc::new_peer_connection(None).unwrap();
    // No track added — must resolve Ok rather than panic or reject.
    apply_tier(&pc, Tier::High).await.unwrap();
}

#[wasm_bindgen_test]
fn is_auto_polling_reflects_whether_a_viewers_auto_poll_is_registered() {
    let conn = crate::session::RoomSession::new();
    assert!(!is_auto_polling(&conn, "viewer-1"));

    conn.quality_auto_intervals
        .borrow_mut()
        .insert("viewer-1".to_string(), 42);
    assert!(is_auto_polling(&conn, "viewer-1"));

    // A fixed-tier switch tears the poll down — `is_auto_polling` must then
    // report false so renegotiation won't re-assert `High` over the choice.
    stop_auto_polling(&conn, "viewer-1");
    assert!(!is_auto_polling(&conn, "viewer-1"));
}
