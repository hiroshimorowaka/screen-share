use super::SharingState;

wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test::wasm_bindgen_test]
fn default_is_idle_and_reports_not_sharing() {
    let state = SharingState::default();
    assert!(!state.is_sharing());
    assert!(state.stream().is_none());
}

#[wasm_bindgen_test::wasm_bindgen_test]
fn sharing_reports_sharing_and_exposes_its_stream() {
    let stream = web_sys::MediaStream::new().unwrap();
    let stream_id = stream.id();
    let state = SharingState::Sharing { stream };

    assert!(state.is_sharing());
    assert_eq!(
        state.stream().map(web_sys::MediaStream::id),
        Some(stream_id)
    );
}

#[wasm_bindgen_test::wasm_bindgen_test]
fn take_returns_the_stream_and_leaves_idle_behind() {
    let stream = web_sys::MediaStream::new().unwrap();
    let stream_id = stream.id();
    let mut state = SharingState::Sharing { stream };

    let taken = state.take();

    assert_eq!(taken.map(|s| s.id()), Some(stream_id));
    assert!(!state.is_sharing());
    assert!(state.stream().is_none());
}

#[wasm_bindgen_test::wasm_bindgen_test]
fn take_on_idle_returns_none_and_stays_idle() {
    let mut state = SharingState::default();

    assert!(state.take().is_none());
    assert!(!state.is_sharing());
}
