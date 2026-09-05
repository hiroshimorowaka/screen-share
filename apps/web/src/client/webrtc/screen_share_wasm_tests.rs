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
fn display_media_constraints_ask_for_surface_matched_audio_in_a_plain_browser() {
    let constraints = display_media_constraints(false);

    let audio = constraints.get_audio();
    assert!(
        audio.is_object(),
        "audio is a constraints object, not a bare bool"
    );
    assert_eq!(
        js_sys::Reflect::get(&audio, &JsValue::from_str("restrictOwnAudio"))
            .unwrap()
            .as_bool(),
        Some(true),
        "never captures this tab's (the room page's) own audio"
    );

    assert_eq!(
        js_sys::Reflect::get(&constraints, &JsValue::from_str("systemAudio"))
            .unwrap()
            .as_string(),
        Some("include".to_string()),
        "offers system audio for a full-screen share"
    );
    assert_eq!(
        js_sys::Reflect::get(&constraints, &JsValue::from_str("windowAudio"))
            .unwrap()
            .as_string(),
        Some("window".to_string()),
        "offers only that window's own audio for a window share"
    );
}
