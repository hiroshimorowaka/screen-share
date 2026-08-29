//! Browser (`wasm32`) tests for `audio_health::probe_share_audio` — the
//! Web Audio wiring, which only runs in a real browser. Split out so
//! `.cargo/mutants.toml`'s `**/*_wasm_tests.rs` exclusion covers it.

use wasm_bindgen::JsCast;
use wasm_bindgen_test::*;

use super::*;

wasm_bindgen_test_configure!(run_in_browser);

fn empty_stream() -> web_sys::MediaStream {
    web_sys::MediaStream::new().unwrap()
}

fn stream_with_silent_audio_track() -> web_sys::MediaStream {
    let generator = web_sys::MediaStreamTrackGenerator::new(
        &web_sys::MediaStreamTrackGeneratorInit::new("audio"),
    )
    .unwrap();
    let track: web_sys::MediaStreamTrack = generator.unchecked_into();
    let stream = web_sys::MediaStream::new().unwrap();
    stream.add_track(&track);
    stream
}

#[wasm_bindgen_test]
async fn no_track_with_audio_expected_is_capture_failed() {
    assert_eq!(
        probe_share_audio(&empty_stream(), true).await,
        AudioHealth::CaptureFailed
    );
}

#[wasm_bindgen_test]
async fn no_track_without_audio_expected_is_not_shared() {
    assert_eq!(
        probe_share_audio(&empty_stream(), false).await,
        AudioHealth::NotShared
    );
}

#[wasm_bindgen_test]
async fn a_present_but_silent_track_is_reported_silent() {
    // The generator track is never fed a sample, so every probe block is
    // silence.
    assert_eq!(
        probe_share_audio(&stream_with_silent_audio_track(), true).await,
        AudioHealth::Silent
    );
}
