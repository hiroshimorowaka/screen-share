//! Browser (`wasm32`) tests for `webrtc::screen_share` — the
//! `getDisplayMedia` constraints and support probe.

use wasm_bindgen_test::*;

use super::*;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
fn is_display_media_supported_is_true_in_a_modern_browser() {
    assert!(is_display_media_supported());
}

#[wasm_bindgen_test]
fn display_media_constraints_ask_only_for_video_in_the_desktop_shell() {
    let constraints = display_media_constraints(true);
    assert!(constraints.get_video().is_truthy(), "video is requested");
    assert!(
        constraints.get_audio().is_undefined(),
        "the desktop shell captures audio through its own backend, not getDisplayMedia"
    );
}

#[wasm_bindgen_test]
fn display_media_constraints_ask_for_tab_audio_in_a_plain_browser() {
    let constraints = display_media_constraints(false);
    assert_eq!(
        constraints.get_audio().as_bool(),
        Some(true),
        "a plain browser tab asks getDisplayMedia for the picker's tab audio too"
    );
}
