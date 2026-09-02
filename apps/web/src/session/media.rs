use leptos::prelude::*;

use crate::session::RoomSession;

#[cfg(not(feature = "hydrate"))]
pub(crate) fn share_supported() -> bool {
    true
}

#[cfg(feature = "hydrate")]
pub(crate) fn share_supported() -> bool {
    crate::infra::webrtc::is_display_media_supported()
}

/// Whether a share started here can carry audio at all. The desktop shell
/// captures system audio through its own backend; a plain browser tab can
/// still capture *its own* tab audio via the `getDisplayMedia` picker. Only
/// a browser without `getDisplayMedia` (which also can't share video) has
/// no way to — the audio-quality / mute controls stay hidden there.
///
/// SSR can't know the browser, so it assumes the capable case (like
/// [`share_supported`]) and lets the `hydrate` value below correct it —
/// keeping the server and first client render structurally identical for
/// the common browser, which is the one that does have `getDisplayMedia`.
#[cfg(not(feature = "hydrate"))]
pub(crate) fn sharing_can_have_audio() -> bool {
    true
}

#[cfg(feature = "hydrate")]
pub(crate) fn sharing_can_have_audio() -> bool {
    crate::infra::webrtc::is_desktop_app() || crate::infra::webrtc::is_display_media_supported()
}

#[cfg(not(feature = "hydrate"))]
pub(crate) fn share_toggle_handler(
    _conn: RoomSession,
    _is_sharing: ReadSignal<bool>,
    _set_is_sharing: WriteSignal<bool>,
    _own_preview_hidden: RwSignal<bool>,
    _set_status: WriteSignal<String>,
    _my_peer_id: ReadSignal<Option<String>>,
    _expanded: RwSignal<Option<String>>,
) -> impl Fn(leptos::ev::MouseEvent) + Clone + 'static {
    move |_| {}
}

#[cfg(feature = "hydrate")]
pub(crate) fn share_toggle_handler(
    conn: RoomSession,
    is_sharing: ReadSignal<bool>,
    set_is_sharing: WriteSignal<bool>,
    own_preview_hidden: RwSignal<bool>,
    set_status: WriteSignal<String>,
    my_peer_id: ReadSignal<Option<String>>,
    expanded: RwSignal<Option<String>>,
) -> impl Fn(leptos::ev::MouseEvent) + Clone + 'static {
    move |_| {
        if is_sharing.get_untracked() {
            stop_sharing(
                &conn,
                set_is_sharing,
                own_preview_hidden,
                expanded,
                my_peer_id,
            );
        } else {
            // A manual click that gets cancelled just leaves the member
            // sitting in the room unshared, same as before — only the
            // quick-share auto-trigger needs to react to a cancelled pick.
            start_sharing(
                conn.clone(),
                set_is_sharing,
                own_preview_hidden,
                set_status,
                my_peer_id,
                expanded,
                || {},
            );
        }
    }
}

/// Hooks the first track of `stream` up to `stop_sharing`, so that the
/// browser's own "Stop sharing" control (which fires `onended` on the track
/// directly, bypassing our buttons) tears the share down the same way our
/// UI does. Shared by `start_sharing` and `switch_source_handler`.
#[cfg(feature = "hydrate")]
fn attach_native_stop_listener(
    stream: &web_sys::MediaStream,
    conn: RoomSession,
    set_is_sharing: WriteSignal<bool>,
    own_preview_hidden: RwSignal<bool>,
    expanded: RwSignal<Option<String>>,
    my_peer_id: ReadSignal<Option<String>>,
) {
    use wasm_bindgen::JsCast;
    use web_sys::MediaStreamTrack;

    let Ok(track) = stream.get_tracks().get(0).dyn_into::<MediaStreamTrack>() else {
        return;
    };
    let callback_conn = conn.clone();
    let onended = wasm_bindgen::prelude::Closure::<dyn FnMut()>::new(move || {
        stop_sharing(
            &callback_conn,
            set_is_sharing,
            own_preview_hidden,
            expanded,
            my_peer_id,
        );
    });
    track.set_onended(Some(onended.as_ref().unchecked_ref()));
    store_local_capture_callback(&conn, onended);
}

/// Stores the native-"Stop sharing" listener on `conn`, replacing any
/// previous one. The old closure is dropped only after the current call
/// stack unwinds: on a source switch the listener being replaced is the
/// one whose `onended` fired and is still on the stack (it runs
/// `stop_sharing`), and dropping a `Closure` from inside its own body
/// would free a box that's still executing.
#[cfg(feature = "hydrate")]
fn store_local_capture_callback(
    conn: &RoomSession,
    callback: wasm_bindgen::prelude::Closure<dyn FnMut()>,
) {
    let previous = conn.local_capture_callback.borrow_mut().replace(callback);
    defer_drop_capture_callback(previous);
}

/// Clears the stored native-stop listener (share teardown). Same
/// deferred-drop reasoning as [`store_local_capture_callback`].
#[cfg(feature = "hydrate")]
fn clear_local_capture_callback(conn: &RoomSession) {
    let previous = conn.local_capture_callback.borrow_mut().take();
    defer_drop_capture_callback(previous);
}

#[cfg(feature = "hydrate")]
fn defer_drop_capture_callback(previous: Option<wasm_bindgen::prelude::Closure<dyn FnMut()>>) {
    if let Some(previous) = previous {
        wasm_bindgen_futures::spawn_local(async move { drop(previous) });
    }
}

/// Points `<video id="{element_id}">` at `stream` and starts playback,
/// idempotently. `ontrack` fires once per track (a shared tab with audio
/// gives video + audio, so twice); without the identity check the second
/// `set_src_object` aborts the first `play()` and Chrome logs
/// `AbortError: The play() request was interrupted by a new load request`.
/// Any `AbortError` that a genuine rapid swap still produces is awaited and
/// dropped rather than left as an unhandled promise rejection.
#[cfg(feature = "hydrate")]
pub(crate) fn play_stream_in(element_id: &str, stream: &web_sys::MediaStream, muted: bool) {
    use wasm_bindgen::JsCast;

    let Some(element) = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.get_element_by_id(element_id))
    else {
        return;
    };
    let video: web_sys::HtmlVideoElement = element.unchecked_into();
    if muted {
        video.set_muted(true);
    }
    if video.src_object().as_ref() != Some(stream) {
        video.set_src_object(Some(stream));
    }
    if let Ok(promise) = video.play() {
        wasm_bindgen_futures::spawn_local(async move {
            let _ = wasm_bindgen_futures::JsFuture::from(promise).await;
        });
    }
}

/// [`play_stream_in`] for the sharer's own always-muted self-preview.
#[cfg(feature = "hydrate")]
fn play_stream_in_self_preview(peer_id: &str, stream: &web_sys::MediaStream) {
    play_stream_in(&format!("video-self-{peer_id}"), stream, true);
}

/// Requests the display picker and, once a stream comes back, wires it up
/// as this member's outgoing share. Split out of `share_toggle_handler` so
/// the quick-share auto-trigger (`RoomPage`'s `quick_share`-driven effect)
/// can start a share without a real click event to hang a handler off of.
/// `on_cancelled` runs if the user closes the picker without choosing
/// anything — the quick-share flow uses it to leave the room instead of
/// sitting in it, hidden and unshared, forever.
#[cfg(feature = "hydrate")]
pub(crate) fn start_sharing(
    conn: RoomSession,
    set_is_sharing: WriteSignal<bool>,
    own_preview_hidden: RwSignal<bool>,
    set_status: WriteSignal<String>,
    my_peer_id: ReadSignal<Option<String>>,
    expanded: RwSignal<Option<String>>,
    on_cancelled: impl Fn() + 'static,
) {
    use leptos::task::spawn_local;

    use crate::infra::webrtc::capture_display;
    use screen_share_protocol::ClientMessage;

    let my_peer_id_value = my_peer_id.get_untracked();

    spawn_local(async move {
        let stream = match capture_display().await {
            Ok(stream) => stream,
            // Cancelling the OS picker rejects here; keep the status clean.
            Err(_) => {
                set_status.set("Conectado.".to_string());
                on_cancelled();
                return;
            }
        };

        // `play_stream_in` forces `.muted` on the element (the `muted`
        // attribute only sets the parse-time default, so a stream whose
        // audio track is attached later would still play out loud) — the
        // sharer must never hear their own shared audio.
        if let Some(peer_id) = my_peer_id_value.as_deref() {
            play_stream_in_self_preview(peer_id, &stream);
        }
        // Keep `set_is_sharing` and the `local_stream` assignment below in
        // the same synchronous run, with no `.await` between them: the
        // audio self-test effect in `RoomPage` reacts to `is_sharing` and
        // then reads `local_stream`, and would see it empty otherwise.
        set_is_sharing.set(true);

        attach_native_stop_listener(
            &stream,
            conn.clone(),
            set_is_sharing,
            own_preview_hidden,
            expanded,
            my_peer_id,
        );

        if let Some(ws) = conn.ws.borrow().as_ref() {
            ws.send(&ClientMessage::StartShare);
        }

        // Hold the exact `stream` captured above, not a clone: if a clone
        // is stored and this original drops here, Chrome's native "sharing"
        // indicator stays lit until the tab closes — even though playback
        // and `stop()` keep working fine on the clone.
        *conn.local_stream.borrow_mut() = Some(stream);

        crate::infra::webrtc::notify_desktop_sharing_changed(true);
    });
}

/// The tracks of `stream` split by kind — a fresh `getDisplayMedia` result
/// always has exactly one video track and zero or one audio tracks.
#[cfg(feature = "hydrate")]
fn video_and_audio_tracks(
    stream: &web_sys::MediaStream,
) -> (
    Option<web_sys::MediaStreamTrack>,
    Option<web_sys::MediaStreamTrack>,
) {
    use wasm_bindgen::JsCast;

    let mut video = None;
    let mut audio = None;
    for entry in stream.get_tracks().iter() {
        let Ok(track) = entry.dyn_into::<web_sys::MediaStreamTrack>() else {
            continue;
        };
        match track.kind().as_str() {
            "video" if video.is_none() => video = Some(track),
            "audio" if audio.is_none() => audio = Some(track),
            _ => {}
        }
    }
    (video, audio)
}

/// Swaps the tracks every viewer connection is sending over to
/// `new_stream`'s, via `RTCRtpSender.replaceTrack` — no renegotiation, so
/// viewers see a source change, not a reconnect. The video sender takes
/// the new video track; the audio sender takes the new audio track, or is
/// cleared (`replaceTrack(null)`) when the new source carries none — a
/// switch that drops audio must actually stop sending it, not leave the
/// old (now stopped) track on the wire. Returns how many senders changed.
/// Sender encoding params (tier, audio preset) survive `replaceTrack`.
///
/// Iterates transceivers, not `getSenders()`, so the still-track-less
/// audio m-line reserved by `webrtc::reserve_audio_mline` (a share that
/// started silent) is matched too — that's the sender a switch that
/// *gains* audio must fill.
#[cfg(feature = "hydrate")]
pub(crate) async fn replace_outgoing_tracks(
    conn: &RoomSession,
    new_stream: &web_sys::MediaStream,
) -> usize {
    use wasm_bindgen::JsCast;
    use wasm_bindgen_futures::JsFuture;

    let (new_video, new_audio) = video_and_audio_tracks(new_stream);

    // `Option<Option<track>>`: outer `None` = not a sender we touch; inner
    // `None` = clear this sender.
    let mut replacements: Vec<(web_sys::RtcRtpSender, Option<web_sys::MediaStreamTrack>)> =
        Vec::new();
    for pc in conn.outgoing.borrow().values() {
        for entry in pc.get_transceivers().iter() {
            let transceiver: web_sys::RtcRtpTransceiver = entry.unchecked_into();
            let sender = transceiver.sender();
            // The sender's own track kind once it has one, otherwise the
            // transceiver's media kind (its receiver track always carries
            // it) — so a reserved audio sender with no track yet still matches.
            let kind = sender
                .track()
                .map(|t| t.kind())
                .unwrap_or_else(|| transceiver.receiver().track().kind());
            match kind.as_str() {
                "video" => replacements.push((sender, new_video.clone())),
                "audio" => replacements.push((sender, new_audio.clone())),
                _ => {}
            }
        }
    }

    let mut swapped = 0;
    for (sender, track) in replacements {
        if JsFuture::from(sender.replace_track(track.as_ref()))
            .await
            .is_ok()
        {
            swapped += 1;
        }
    }
    swapped
}

/// Releases this member's outgoing share: stops the desktop audio loopback,
/// detaches the shared tracks from every viewer connection, stops and
/// unlinks the local capture tracks, clears the self-preview `<video>`,
/// closes the viewer connections, halts any Auto-quality polling, and tells
/// the server. Everything here is browser/registry teardown with no
/// reactive state — [`stop_sharing`] layers the `is_sharing` / preview /
/// focus signal resets on top, and `leave_room` reuses it so leaving
/// mid-share doesn't strand Chrome's native "you're sharing" indicator.
#[cfg(feature = "hydrate")]
pub(crate) fn teardown_local_share(conn: &RoomSession, my_peer_id: Option<&str>) {
    use wasm_bindgen::JsCast;
    // `wasm_bindgen_futures::spawn_local`, not `leptos::task`'s: this is a
    // detached cleanup future that touches no reactive state, and the
    // wasm-bindgen primitive needs only the JS microtask queue — no global
    // executor, so `teardown_local_share` stays callable from a plain
    // `wasm-bindgen-test`.
    use wasm_bindgen_futures::spawn_local;

    // Always attempt this — it's a no-op in Electron if no audio
    // session was ever started, and this path also runs in a plain
    // browser (no `window.desktopAudio` there), where it's likewise a
    // harmless no-op inside `stop_desktop_audio_loopback` itself.
    spawn_local(async {
        let _ = crate::infra::webrtc::stop_desktop_audio_loopback().await;
    });

    // Chrome keeps its native "sharing" indicator alive as long as any
    // RTCRtpSender still references the track, even after the track itself
    // is stopped — detach it from every viewer's connection first, or the
    // indicator survives `track.stop()` and stacks with the next share.
    for pc in conn.outgoing.borrow().values() {
        for sender in pc.get_senders().iter() {
            let sender: web_sys::RtcRtpSender = sender.unchecked_into();
            pc.remove_track(&sender);
        }
    }

    if let Some(stream) = conn.local_stream.borrow_mut().take() {
        // `stop()` alone marks the track "ended" but leaves it attached to
        // the `MediaStream` object; several Chromium builds only drop the
        // native "sharing" indicator once the track is also detached from
        // the stream via `removeTrack` (this is how Google Meet's own
        // "Stop presenting" avoids the stuck-indicator bug).
        for track in stream.get_tracks().iter() {
            let track: web_sys::MediaStreamTrack = track.unchecked_into();
            track.stop();
            stream.remove_track(&track);
        }
    }
    // The preview `<video>` keeps its `srcObject` pointing at this stream
    // even after the tracks are stopped — on several Chromium builds that
    // alone is enough to keep the native sharing indicator (red dot on the
    // tab + "stop sharing" bar) visible. Clearing `srcObject` removes the
    // last reference and actually releases the indicator.
    if let Some(peer_id) = my_peer_id {
        if let Some(document) = web_sys::window().and_then(|w| w.document()) {
            if let Some(video) = document.get_element_by_id(&format!("video-self-{peer_id}")) {
                let video: web_sys::HtmlVideoElement = video.unchecked_into();
                video.set_src_object(None);
            }
        }
    }
    for (_, pc) in conn.outgoing.borrow_mut().drain() {
        pc.close();
    }
    conn.outgoing_callbacks.borrow_mut().clear();
    clear_local_capture_callback(conn);
    // Every viewer's Auto poll would otherwise keep firing against a
    // connection that's already closed.
    super::quality::stop_all_auto_polling(conn);
    if let Some(ws) = conn.ws.borrow().as_ref() {
        ws.send(&screen_share_protocol::ClientMessage::StopShare);
    }
    crate::infra::webrtc::notify_desktop_sharing_changed(false);
}

#[cfg(feature = "hydrate")]
pub(crate) fn stop_sharing(
    conn: &RoomSession,
    set_is_sharing: WriteSignal<bool>,
    own_preview_hidden: RwSignal<bool>,
    expanded: RwSignal<Option<String>>,
    my_peer_id: ReadSignal<Option<String>>,
) {
    let me = my_peer_id.get_untracked();
    teardown_local_share(conn, me.as_deref());

    set_is_sharing.set(false);
    own_preview_hidden.set(false);
    // The server only sends `PeerStoppedSharing` to the other members, so
    // nobody else collapses the focus for us — without this the grid would
    // stay stuck in `grid--focused` with an empty card.
    expanded.update(|current| {
        if current.as_deref() == me.as_deref() {
            *current = None;
        }
    });
}

#[allow(clippy::too_many_arguments)]
#[cfg(not(feature = "hydrate"))]
pub(crate) fn switch_source_handler(
    _conn: RoomSession,
    _set_is_sharing: WriteSignal<bool>,
    _own_preview_hidden: RwSignal<bool>,
    _set_status: WriteSignal<String>,
    _my_peer_id: ReadSignal<Option<String>>,
    _expanded: RwSignal<Option<String>>,
    _audio_muted: ReadSignal<bool>,
    _video_mode: ReadSignal<crate::session::video_mode::VideoMode>,
    _share_generation: RwSignal<u32>,
) -> impl Fn(leptos::ev::MouseEvent) + Clone + 'static {
    move |_| {}
}

/// Re-opens the display picker and swaps the running share over to the
/// newly chosen source without dropping any viewer connection (see
/// `replace_outgoing_tracks`). Cancelling the picker leaves the current
/// share untouched. The new video/audio tracks start fresh, so the
/// sharer's `audio_muted` and `video_mode` are re-applied over them —
/// otherwise a switch would silently un-mute and reset the encoder hint.
// Eight reactive handles, each distinct and all needed to rebuild the
// share and re-assert the sharer's preferences over the new tracks —
// bundling them into a struct would just move the same list one level down.
#[allow(clippy::too_many_arguments)]
#[cfg(feature = "hydrate")]
pub(crate) fn switch_source_handler(
    conn: RoomSession,
    set_is_sharing: WriteSignal<bool>,
    own_preview_hidden: RwSignal<bool>,
    set_status: WriteSignal<String>,
    my_peer_id: ReadSignal<Option<String>>,
    expanded: RwSignal<Option<String>>,
    audio_muted: ReadSignal<bool>,
    video_mode: ReadSignal<crate::session::video_mode::VideoMode>,
    share_generation: RwSignal<u32>,
) -> impl Fn(leptos::ev::MouseEvent) + Clone + 'static {
    use leptos::task::spawn_local;
    use wasm_bindgen::JsCast;

    use crate::infra::webrtc::{capture_display, stop_desktop_audio_loopback};

    move |_| {
        // Nothing to switch if we aren't the one sharing.
        if conn.local_stream.borrow().is_none() {
            return;
        }
        let conn = conn.clone();
        let my_peer_id_value = my_peer_id.get_untracked();

        spawn_local(async move {
            // Stop the current outgoing audio track *before* its capture
            // device goes away. The desktop loopback is torn down and
            // re-created for the new source below; Chromium reroutes a
            // still-live `getUserMedia` audio track whose device vanishes
            // to the default microphone, and viewers then hear the mic
            // (it stuck until the share was fully restarted). A stopped
            // track can't be rerouted.
            if let Some(stream) = conn.local_stream.borrow().as_ref() {
                for entry in stream.get_tracks().iter() {
                    let track: web_sys::MediaStreamTrack = entry.unchecked_into();
                    if track.kind() == "audio" {
                        track.stop();
                    }
                }
            }

            // Release the old desktop audio loopback first so re-capturing
            // doesn't stack a second one; a no-op in a plain browser.
            let _ = stop_desktop_audio_loopback().await;

            let new_stream = match capture_display().await {
                Ok(stream) => stream,
                Err(_) => {
                    set_status.set("Conectado.".to_string());
                    return;
                }
            };

            replace_outgoing_tracks(&conn, &new_stream).await;

            if let Some(peer_id) = my_peer_id_value.as_deref() {
                play_stream_in_self_preview(peer_id, &new_stream);
            }

            attach_native_stop_listener(
                &new_stream,
                conn.clone(),
                set_is_sharing,
                own_preview_hidden,
                expanded,
                my_peer_id,
            );

            // Release the stream we switched away from (stop + detach its
            // tracks — the `stop_sharing` indicator dance), then store the
            // *exact* new stream. Storing a clone and letting the original
            // wrapper drop here leaves Chrome's native "sharing" indicator
            // lit for that capture forever, so every switch stacked
            // another bar (see the matching note in `start_sharing`).
            let old_stream = conn.local_stream.borrow_mut().take();
            if let Some(old_stream) = old_stream {
                for entry in old_stream.get_tracks().iter() {
                    let track: web_sys::MediaStreamTrack = entry.unchecked_into();
                    track.stop();
                    old_stream.remove_track(&track);
                }
            }
            *conn.local_stream.borrow_mut() = Some(new_stream);

            super::audio::set_shared_audio_muted(&conn, audio_muted.get_untracked());
            super::video_mode::apply_video_mode_to_all(&conn, video_mode.get_untracked()).await;
            share_generation.update(|generation| *generation = generation.wrapping_add(1));
            set_status.set("Conectado.".to_string());
        });
    }
}

#[cfg(all(test, target_arch = "wasm32", feature = "hydrate"))]
#[path = "media_wasm_tests.rs"]
mod wasm_tests;
