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
    assert!(!crate::client::desktop_bridge::is_desktop_app());
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
    let conn = crate::room::RoomSession::new();

    let old_video = track("video");
    let old_stream = web_sys::MediaStream::new().unwrap();
    old_stream.add_track(&old_video);

    // Two viewer connections, each sending the old video track.
    for peer in ["viewer-a", "viewer-b"] {
        let pc = crate::client::webrtc::new_peer_connection(None).unwrap();
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
    let conn = crate::room::RoomSession::new();

    let shared = stream_of(&["video", "audio"]);
    let (shared_video, _) = video_and_audio_tracks(&shared);
    let shared_video = shared_video.unwrap();
    for peer in ["viewer-a", "viewer-b"] {
        let pc = crate::client::webrtc::new_peer_connection(None).unwrap();
        pc.add_track_0(&shared_video, &shared);
        conn.outgoing.borrow_mut().insert(peer.to_string(), pc);
        // A live Auto poll for this viewer — teardown must stop it without
        // deadlocking on `quality_auto_intervals` (the id is a throwaway;
        // `clear_interval` no-ops on an unknown handle).
        conn.quality_auto_intervals.borrow_mut().insert(
            peer.to_string(),
            crate::room::quality::AutoPoll::for_test(0),
        );
        conn.outgoing_callbacks.borrow_mut().insert(
            peer.to_string(),
            crate::room::messages::PeerCallbacks::empty_for_test(),
        );
    }
    *conn.sharing.borrow_mut() = SharingState::Sharing {
        stream: shared.clone(),
    };

    teardown_local_share(&conn, None);

    // The registry-level teardown: the capture handle is dropped and every
    // viewer connection is closed and removed. (Stopping/detaching the
    // individual tracks — what actually releases Chrome's native "sharing"
    // indicator — needs real `getDisplayMedia` tracks; a synthetic
    // `MediaStreamTrackGenerator` doesn't honour `stop()`/`removeTrack`
    // here, so that step is covered by the `e2e-web` leave-while-sharing
    // flow instead.)
    assert!(
        !conn.sharing.borrow().is_sharing(),
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
async fn replace_outgoing_tracks_clears_the_audio_sender_when_the_new_stream_has_no_audio() {
    let conn = crate::room::RoomSession::new();

    let old = stream_of(&["video", "audio"]);
    let (old_video, old_audio) = video_and_audio_tracks(&old);
    let pc = crate::client::webrtc::new_peer_connection(None).unwrap();
    pc.add_track_0(&old_video.unwrap(), &old);
    pc.add_track_0(&old_audio.clone().unwrap(), &old);
    conn.outgoing.borrow_mut().insert("viewer".to_string(), pc);

    // New source is video-only: the audio sender is cleared, not left
    // holding the old (by then stopped) track (bug: a desktop switch that
    // dropped audio kept sending a dead/mic track).
    let swapped = replace_outgoing_tracks(&conn, &stream_of(&["video"])).await;

    assert_eq!(
        swapped, 2,
        "the video sender is swapped and the audio sender cleared"
    );
    let pc = conn.outgoing.borrow().values().next().unwrap().clone();
    let any_audio_track = pc.get_senders().iter().any(|entry| {
        let sender: web_sys::RtcRtpSender = entry.unchecked_into();
        sender.track().map(|t| t.kind()) == Some("audio".to_string())
    });
    assert!(!any_audio_track, "no sender still carries an audio track");
}

#[wasm_bindgen_test]
async fn replace_outgoing_tracks_fills_a_reserved_audio_mline_when_a_switch_gains_audio() {
    let conn = crate::room::RoomSession::new();

    // A viewer connection for a share that started silent: the video track
    // plus the reserved (track-less) `sendonly` audio m-line, exactly what
    // `WatchRequested` builds now.
    let started_silent = stream_of(&["video"]);
    let (silent_video, _) = video_and_audio_tracks(&started_silent);
    let pc = crate::client::webrtc::new_peer_connection(None).unwrap();
    pc.add_track_0(&silent_video.unwrap(), &started_silent);
    crate::client::webrtc::reserve_audio_mline(&pc, &started_silent);
    conn.outgoing.borrow_mut().insert("viewer".to_string(), pc);

    // "Trocar fonte" to a source that now carries audio.
    let with_audio = stream_of(&["video", "audio"]);
    let (_, switched_in_audio) = video_and_audio_tracks(&with_audio);
    let switched_in_audio_id = switched_in_audio.unwrap().id();

    let swapped = replace_outgoing_tracks(&conn, &with_audio).await;

    assert_eq!(
        swapped, 2,
        "the video sender and the reserved audio sender both take a track"
    );
    let pc = conn.outgoing.borrow().values().next().unwrap().clone();
    let carries_switched_in_audio = pc.get_senders().iter().any(|entry| {
        let sender: web_sys::RtcRtpSender = entry.unchecked_into();
        sender.track().map(|t| t.id()) == Some(switched_in_audio_id.clone())
    });
    assert!(
        carries_switched_in_audio,
        "the reserved audio sender now carries the switched-in audio track"
    );
}

#[wasm_bindgen_test]
fn reserve_audio_mline_adds_one_sendonly_audio_transceiver() {
    let pc = crate::client::webrtc::new_peer_connection(None).unwrap();
    let stream = stream_of(&["video"]);
    pc.add_track_0(&video_and_audio_tracks(&stream).0.unwrap(), &stream);

    crate::client::webrtc::reserve_audio_mline(&pc, &stream);

    let audio_transceivers = pc
        .get_transceivers()
        .iter()
        .filter(|entry| {
            let transceiver: web_sys::RtcRtpTransceiver = entry.clone().unchecked_into();
            transceiver.receiver().track().kind() == "audio"
        })
        .count();
    assert_eq!(audio_transceivers, 1, "one audio m-line is reserved");
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
    crate::room::media::play_stream_in("test-play-target", &stream, false);
    crate::room::media::play_stream_in("test-play-target", &stream, false);

    let v: web_sys::HtmlVideoElement = video.unchecked_into();
    assert_eq!(v.src_object().as_ref(), Some(&stream));
    v.remove();
}

#[wasm_bindgen_test]
fn native_stop_listener_is_retained_on_the_session_and_cleared_on_teardown() {
    let conn = crate::room::RoomSession::new();

    // Previously `Closure::forget`'d once per share and once per source
    // switch, never freed (finding F08a).
    let cb = wasm_bindgen::prelude::Closure::<dyn FnMut()>::new(|| {});
    store_local_capture_callback(&conn, cb);
    assert!(
        conn.local_capture_callback.borrow().is_some(),
        "the listener is kept on the session, not forgotten"
    );

    // A source switch replaces the single slot rather than stacking.
    let cb2 = wasm_bindgen::prelude::Closure::<dyn FnMut()>::new(|| {});
    store_local_capture_callback(&conn, cb2);
    assert!(conn.local_capture_callback.borrow().is_some());

    clear_local_capture_callback(&conn);
    assert!(
        conn.local_capture_callback.borrow().is_none(),
        "share teardown clears the stored listener"
    );
}

/// A `DisplayCapture` that resolves to a given stream instead of prompting
/// a real picker — headless Chrome has no display to capture, so this is
/// the only way `start_sharing`'s happy path (as opposed to the
/// always-taken cancelled-picker branch) gets exercised outside `e2e-web`.
#[derive(Clone)]
struct FakeDisplayCapture {
    stream: web_sys::MediaStream,
}

impl DisplayCapture for FakeDisplayCapture {
    async fn capture(&self) -> Result<web_sys::MediaStream, wasm_bindgen::JsValue> {
        // `Clone::clone`, not `self.stream.clone()`: `MediaStream` has an
        // inherent `clone()` bound to the DOM's `MediaStream.clone()) —
        // an actual new stream with a new id — which method-call syntax
        // resolves to over the trait. UFCS forces the trait's cheap
        // reference clone instead, so the test can still recognize the
        // exact stream it handed in by `id()`.
        Ok(Clone::clone(&self.stream))
    }
}

/// A `DisplayCapture` that always rejects, like a real cancelled picker.
#[derive(Clone)]
struct RejectingDisplayCapture;

impl DisplayCapture for RejectingDisplayCapture {
    async fn capture(&self) -> Result<web_sys::MediaStream, wasm_bindgen::JsValue> {
        Err(wasm_bindgen::JsValue::from_str("cancelled"))
    }
}

/// `start_sharing` spawns its work via `leptos::task::spawn_local`, which
/// needs a global executor initialized first — the real app gets this for
/// free from `leptos::mount::hydrate_body`, but the wasm test harness
/// never calls that. Idempotent (`init_wasm_bindgen` errors, harmlessly,
/// on a second call), so every test in this file can call it.
fn ensure_executor() {
    let _ = any_spawner::Executor::init_wasm_bindgen();
}

/// `start_sharing` only ever awaits once (`capture.capture()`) before
/// finishing synchronously — see the comment above `set_is_sharing.set(true)`
/// in `start_sharing` itself. Yielding a few real microtask turns is
/// enough for its `spawn_local`'d task to run to completion.
async fn flush_microtasks() {
    for _ in 0..5 {
        let _ = wasm_bindgen_futures::JsFuture::from(js_sys::Promise::resolve(
            &wasm_bindgen::JsValue::UNDEFINED,
        ))
        .await;
    }
}

#[wasm_bindgen_test]
async fn start_sharing_with_a_resolved_capture_marks_sharing_and_stores_the_stream() {
    ensure_executor();
    let owner = Owner::new();
    let (is_sharing, set_is_sharing) = owner.with(|| signal(false));
    let (my_peer_id, _) = owner.with(|| signal(None::<String>));
    let (_status, set_status) = owner.with(|| signal(String::new()));
    let own_preview_hidden = owner.with(|| RwSignal::new(false));
    let expanded = owner.with(|| RwSignal::new(None::<String>));

    let conn = crate::room::RoomSession::new();
    let stream = stream_of(&["video"]);
    let stream_id = stream.id();
    let capture = FakeDisplayCapture { stream };

    start_sharing(
        capture,
        conn.clone(),
        set_is_sharing,
        own_preview_hidden,
        set_status,
        my_peer_id,
        expanded,
        || panic!("a resolved capture must not run on_cancelled"),
    );
    flush_microtasks().await;

    assert!(
        is_sharing.get_untracked(),
        "is_sharing flips true once the capture resolves"
    );
    assert!(conn.sharing.borrow().is_sharing());
    assert_eq!(
        conn.sharing.borrow().stream().map(web_sys::MediaStream::id),
        Some(stream_id),
        "the exact captured stream is stored, not a clone-of-a-clone"
    );
}

#[wasm_bindgen_test]
async fn start_sharing_with_a_rejecting_capture_runs_on_cancelled_and_stays_idle() {
    ensure_executor();
    let owner = Owner::new();
    let (is_sharing, set_is_sharing) = owner.with(|| signal(false));
    let (my_peer_id, _) = owner.with(|| signal(None::<String>));
    let (status, set_status) = owner.with(|| signal(String::new()));
    let own_preview_hidden = owner.with(|| RwSignal::new(false));
    let expanded = owner.with(|| RwSignal::new(None::<String>));

    let conn = crate::room::RoomSession::new();
    let cancelled = std::rc::Rc::new(std::cell::Cell::new(false));
    let cancelled_flag = cancelled.clone();

    start_sharing(
        RejectingDisplayCapture,
        conn.clone(),
        set_is_sharing,
        own_preview_hidden,
        set_status,
        my_peer_id,
        expanded,
        move || cancelled_flag.set(true),
    );
    flush_microtasks().await;

    assert!(
        cancelled.get(),
        "on_cancelled runs when the picker (here, the fake) rejects"
    );
    assert!(!is_sharing.get_untracked(), "sharing never starts");
    assert!(!conn.sharing.borrow().is_sharing());
    assert_eq!(status.get_untracked(), "Conectado.");
}
