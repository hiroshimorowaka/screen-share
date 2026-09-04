//! Browser (`wasm32`) tests for `audio` — the parts that touch real
//! `MediaStreamTrack`s. Split out so `.cargo/mutants.toml`'s
//! `**/*_wasm_tests.rs` exclusion covers it.

use wasm_bindgen::JsCast;
use wasm_bindgen_test::*;

use super::*;
use crate::room::{RoomSession, SharingState};

wasm_bindgen_test_configure!(run_in_browser);

fn track(kind: &str) -> web_sys::MediaStreamTrack {
    web_sys::MediaStreamTrackGenerator::new(&web_sys::MediaStreamTrackGeneratorInit::new(kind))
        .unwrap()
        .unchecked_into()
}

#[wasm_bindgen_test]
fn set_shared_audio_muted_toggles_enabled_on_the_audio_track_only() {
    let conn = RoomSession::new();
    let video = track("video");
    let audio = track("audio");
    let stream = web_sys::MediaStream::new().unwrap();
    stream.add_track(&video);
    stream.add_track(&audio);
    *conn.sharing.borrow_mut() = SharingState::Sharing { stream };

    set_shared_audio_muted(&conn, true);
    assert!(!audio.enabled(), "audio muted");
    assert!(video.enabled(), "video untouched");

    set_shared_audio_muted(&conn, false);
    assert!(audio.enabled(), "audio unmuted again");
}

#[wasm_bindgen_test]
fn set_shared_audio_muted_is_a_noop_when_not_sharing() {
    let conn = RoomSession::new();
    // No local stream — must not panic.
    set_shared_audio_muted(&conn, true);
}
