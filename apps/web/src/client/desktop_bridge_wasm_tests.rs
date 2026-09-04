//! Browser (`wasm32`) tests for `desktop_bridge` — the Electron
//! tray/notification bridge detection, which only runs inside a real
//! browser.

use wasm_bindgen_test::*;

use super::*;

wasm_bindgen_test_configure!(run_in_browser);

fn win() -> web_sys::Window {
    web_sys::window().unwrap()
}

fn set_window_prop(name: &str, value: &wasm_bindgen::JsValue) {
    js_sys::Reflect::set(&win(), &wasm_bindgen::JsValue::from_str(name), value).unwrap();
}

fn clear_window_prop(name: &str) {
    let _ = js_sys::Reflect::delete_property(&win(), &wasm_bindgen::JsValue::from_str(name));
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
