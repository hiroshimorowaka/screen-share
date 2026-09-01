//! Browser (`wasm32`) tests for the live source-switch plumbing in
//! `media` — `replace_outgoing_tracks` and its track-splitting helper.
//! Split out so `.cargo/mutants.toml`'s `**/*_wasm_tests.rs` exclusion
//! covers it.

use wasm_bindgen::JsCast;
use wasm_bindgen_test::*;

use super::*;

wasm_bindgen_test_configure!(run_in_browser);

fn track(kind: &str) -> web_sys::MediaStreamTrack {
    web_sys::MediaStreamTrackGenerator::new(&web_sys::MediaStreamTrackGeneratorInit::new(kind))
        .unwrap()
        .unchecked_into()
}

fn stream_of(kinds: &[&str]) -> web_sys::MediaStream {
    let stream = web_sys::MediaStream::new().unwrap();
    for kind in kinds {
        stream.add_track(&track(kind));
    }
    stream
}

#[wasm_bindgen_test]
fn sharing_can_have_audio_holds_in_a_plain_browser_that_can_screen_share() {
    // No desktop shell injected here; headless Chrome still supports
    // `getDisplayMedia`, so the tab's own "share tab audio" path qualifies.
    let _ = js_sys::Reflect::delete_property(
        &web_sys::window().unwrap(),
        &wasm_bindgen::JsValue::from_str("desktopAudio"),
    );
    assert!(!crate::infra::webrtc::is_desktop_app());
    assert!(sharing_can_have_audio());
}

#[wasm_bindgen_test]
fn video_and_audio_tracks_splits_a_mixed_stream_by_kind() {
    let (video, audio) = video_and_audio_tracks(&stream_of(&["video", "audio"]));
    assert_eq!(video.map(|t| t.kind()).as_deref(), Some("video"));
    assert_eq!(audio.map(|t| t.kind()).as_deref(), Some("audio"));
}

#[wasm_bindgen_test]
fn video_and_audio_tracks_reports_no_audio_for_a_video_only_stream() {
    let (video, audio) = video_and_audio_tracks(&stream_of(&["video"]));
    assert!(video.is_some());
    assert!(audio.is_none());
}

#[wasm_bindgen_test]
async fn replace_outgoing_tracks_swaps_every_matching_sender() {
    let conn = crate::session::RoomSession::new();

    let old_video = track("video");
    let old_stream = web_sys::MediaStream::new().unwrap();
    old_stream.add_track(&old_video);

    // Two viewer connections, each sending the old video track.
    for peer in ["viewer-a", "viewer-b"] {
        let pc = crate::infra::webrtc::new_peer_connection(None).unwrap();
        pc.add_track_0(&old_video, &old_stream);
        conn.outgoing.borrow_mut().insert(peer.to_string(), pc);
    }

    let new_stream = stream_of(&["video"]);
    let (new_video, _) = video_and_audio_tracks(&new_stream);
    let new_video_id = new_video.unwrap().id();

    let swapped = replace_outgoing_tracks(&conn, &new_stream).await;

    assert_eq!(swapped, 2, "one video sender per connection");
    for pc in conn.outgoing.borrow().values() {
        let sender: web_sys::RtcRtpSender = pc.get_senders().get(0).unchecked_into();
        assert_eq!(
            sender.track().map(|t| t.id()),
            Some(new_video_id.clone()),
            "sender now carries the new track"
        );
    }
}

#[wasm_bindgen_test]
async fn teardown_local_share_releases_the_stream_and_every_viewer_connection() {
    let conn = crate::session::RoomSession::new();

    let shared = stream_of(&["video", "audio"]);
    let (shared_video, _) = video_and_audio_tracks(&shared);
    let shared_video = shared_video.unwrap();
    for peer in ["viewer-a", "viewer-b"] {
        let pc = crate::infra::webrtc::new_peer_connection(None).unwrap();
        pc.add_track_0(&shared_video, &shared);
        conn.outgoing.borrow_mut().insert(peer.to_string(), pc);
        // A live Auto poll for this viewer — teardown must stop it without
        // deadlocking on `quality_auto_intervals` (the id is a throwaway;
        // `clear_interval` no-ops on an unknown handle).
        conn.quality_auto_intervals.borrow_mut().insert(
            peer.to_string(),
            crate::session::quality::AutoPoll::for_test(0),
        );
        conn.outgoing_callbacks.borrow_mut().insert(
            peer.to_string(),
            crate::session::handler::PeerCallbacks::empty_for_test(),
        );
    }
    *conn.local_stream.borrow_mut() = Some(shared.clone());

    teardown_local_share(&conn, None);

    // The registry-level teardown: the capture handle is dropped and every
    // viewer connection is closed and removed. (Stopping/detaching the
    // individual tracks — what actually releases Chrome's native "sharing"
    // indicator — needs real `getDisplayMedia` tracks; a synthetic
    // `MediaStreamTrackGenerator` doesn't honour `stop()`/`removeTrack`
    // here, so that step is covered by the `e2e-web` leave-while-sharing
    // flow instead.)
    assert!(
        conn.local_stream.borrow().is_none(),
        "the capture stream handle is released"
    );
    assert!(
        conn.outgoing.borrow().is_empty(),
        "every viewer connection is dropped"
    );
    assert!(
        conn.quality_auto_intervals.borrow().is_empty(),
        "every Auto poll is stopped"
    );
    assert!(
        conn.outgoing_callbacks.borrow().is_empty(),
        "every viewer connection's callbacks are dropped"
    );
}

#[wasm_bindgen_test]
async fn replace_outgoing_tracks_leaves_audio_senders_alone_when_the_new_stream_has_no_audio() {
    let conn = crate::session::RoomSession::new();

    let old = stream_of(&["video", "audio"]);
    let (old_video, old_audio) = video_and_audio_tracks(&old);
    let pc = crate::infra::webrtc::new_peer_connection(None).unwrap();
    pc.add_track_0(&old_video.unwrap(), &old);
    pc.add_track_0(&old_audio.clone().unwrap(), &old);
    conn.outgoing.borrow_mut().insert("viewer".to_string(), pc);

    // New source is video-only.
    let swapped = replace_outgoing_tracks(&conn, &stream_of(&["video"])).await;

    assert_eq!(swapped, 1, "only the video sender is swapped");
    let pc = conn.outgoing.borrow().values().next().unwrap().clone();
    let audio_still_there = pc.get_senders().iter().any(|entry| {
        let sender: web_sys::RtcRtpSender = entry.unchecked_into();
        sender.track().map(|t| t.kind()) == Some("audio".to_string())
    });
    assert!(audio_still_there, "the old audio sender keeps its track");
}

#[wasm_bindgen_test]
fn play_stream_in_is_idempotent_across_repeat_calls() {
    let doc = web_sys::window().unwrap().document().unwrap();
    let video = doc.create_element("video").unwrap();
    video.set_id("test-play-target");
    doc.body().unwrap().append_child(&video).unwrap();

    let stream = stream_of(&["video", "audio"]);

    // `ontrack` fires once per track — call twice with the same stream and
    // expect no panic and the element left pointing at that stream (the
    // real fix also swallows the `play()` AbortError this would trigger).
    crate::session::media::play_stream_in("test-play-target", &stream, false);
    crate::session::media::play_stream_in("test-play-target", &stream, false);

    let v: web_sys::HtmlVideoElement = video.unchecked_into();
    assert_eq!(v.src_object().as_ref(), Some(&stream));
    v.remove();
}
