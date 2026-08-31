//! Browser (`wasm32`) tests for `webrtc` — the desktop-shell bridge
//! detection and the WebRTC offer/answer plumbing, which only run inside a
//! real browser. Run with:
//!
//! ```text
//! cargo test -p screen_share --target wasm32-unknown-unknown \
//!   --no-default-features --features hydrate
//! ```

use wasm_bindgen_test::*;

use super::*;

wasm_bindgen_test_configure!(run_in_browser);

fn win() -> web_sys::Window {
    web_sys::window().unwrap()
}

fn set_window_prop(name: &str, value: &JsValue) {
    js_sys::Reflect::set(&win(), &JsValue::from_str(name), value).unwrap();
}

fn clear_window_prop(name: &str) {
    let _ = js_sys::Reflect::delete_property(&win(), &JsValue::from_str(name));
}

#[wasm_bindgen_test]
fn is_desktop_app_is_false_in_a_plain_browser_tab() {
    clear_window_prop("desktopAudio");
    assert!(!is_desktop_app());
}

#[wasm_bindgen_test]
fn is_desktop_app_is_true_when_the_shell_injected_desktop_audio() {
    set_window_prop("desktopAudio", &js_sys::Object::new());
    assert!(is_desktop_app());
    clear_window_prop("desktopAudio");
}

#[wasm_bindgen_test]
fn notify_desktop_share_ready_is_a_noop_without_the_bridge() {
    clear_window_prop("desktopAudio");
    clear_window_prop("desktopShare");
    // Must simply not panic when there is no shell to talk to.
    notify_desktop_share_ready("https://example.com/room/x");
}

#[wasm_bindgen_test]
fn notify_desktop_share_ready_calls_the_shell_bridge_with_the_link() {
    // `is_desktop_app()` gates the call, so the marker prop must be set too.
    set_window_prop("desktopAudio", &js_sys::Object::new());

    let bridge = js_sys::Object::new();
    let recorder = js_sys::Function::new_with_args("link", "globalThis.__seen_link = link;");
    js_sys::Reflect::set(&bridge, &JsValue::from_str("linkReady"), &recorder).unwrap();
    set_window_prop("desktopShare", &bridge);

    notify_desktop_share_ready("https://example.com/room/abc");

    let seen = js_sys::Reflect::get(&win(), &JsValue::from_str("__seen_link")).unwrap();
    assert_eq!(
        seen.as_string().as_deref(),
        Some("https://example.com/room/abc")
    );

    clear_window_prop("desktopAudio");
    clear_window_prop("desktopShare");
    clear_window_prop("__seen_link");
}

#[wasm_bindgen_test]
fn notify_desktop_sharing_changed_is_a_noop_without_the_bridge() {
    clear_window_prop("desktopAudio");
    clear_window_prop("desktopShare");
    notify_desktop_sharing_changed(true);
}

#[wasm_bindgen_test]
fn notify_desktop_sharing_changed_forwards_the_boolean_to_the_shell_bridge() {
    set_window_prop("desktopAudio", &js_sys::Object::new());

    let bridge = js_sys::Object::new();
    let recorder = js_sys::Function::new_with_args("live", "globalThis.__seen_sharing = live;");
    js_sys::Reflect::set(&bridge, &JsValue::from_str("sharingChanged"), &recorder).unwrap();
    set_window_prop("desktopShare", &bridge);

    notify_desktop_sharing_changed(true);
    assert_eq!(
        js_sys::Reflect::get(&win(), &JsValue::from_str("__seen_sharing"))
            .unwrap()
            .as_bool(),
        Some(true)
    );
    notify_desktop_sharing_changed(false);
    assert_eq!(
        js_sys::Reflect::get(&win(), &JsValue::from_str("__seen_sharing"))
            .unwrap()
            .as_bool(),
        Some(false)
    );

    clear_window_prop("desktopAudio");
    clear_window_prop("desktopShare");
    clear_window_prop("__seen_sharing");
}

#[wasm_bindgen_test]
fn is_display_media_supported_is_true_in_a_modern_browser() {
    assert!(is_display_media_supported());
}

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
