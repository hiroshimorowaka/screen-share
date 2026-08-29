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

/// Whether a share started here can carry system audio at all — only the
/// desktop shell captures it (a plain browser tab shares video only), so
/// the audio-quality / mute controls are hidden otherwise.
#[cfg(not(feature = "hydrate"))]
pub(crate) fn sharing_can_have_audio() -> bool {
    false
}

#[cfg(feature = "hydrate")]
pub(crate) fn sharing_can_have_audio() -> bool {
    crate::infra::webrtc::is_desktop_app()
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
    let onended = wasm_bindgen::prelude::Closure::<dyn FnMut()>::new(move || {
        stop_sharing(
            &conn,
            set_is_sharing,
            own_preview_hidden,
            expanded,
            my_peer_id,
        );
    });
    track.set_onended(Some(onended.as_ref().unchecked_ref()));
    onended.forget();
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
    use wasm_bindgen::JsCast;

    use crate::infra::webrtc::capture_display;
    use screen_share_protocol::ClientMessage;

    let my_peer_id_value = my_peer_id.get_untracked();
    set_status.set("Selecione a tela para compartilhar...".to_string());

    spawn_local(async move {
        let stream = match capture_display().await {
            Ok(stream) => stream,
            Err(_) => {
                set_status.set("Conectado.".to_string());
                on_cancelled();
                return;
            }
        };

        if let Some(peer_id) = my_peer_id_value.as_deref() {
            if let Some(document) = web_sys::window().and_then(|w| w.document()) {
                if let Some(video) = document.get_element_by_id(&format!("video-self-{peer_id}")) {
                    let video: web_sys::HtmlVideoElement = video.unchecked_into();
                    video.set_src_object(Some(&stream));
                    // The `muted` attribute only sets the element's
                    // *default* muted state at parse time — it doesn't
                    // reflect the live `.muted` property, so a stream
                    // with an audio track attached later can still play
                    // out loud unless this is set explicitly. The
                    // sharer must never hear their own shared audio.
                    video.set_muted(true);
                    let _ = video.play();
                }
            }
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

        // Store the same `stream` handle used above, not a `.clone()` of
        // it — cloning here and letting this original drop at the end of
        // the block was enough, on its own, to keep Chrome's native
        // "sharing" indicator from ever releasing later, even though the
        // clone kept working fine for playback and `stop()`.
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

/// Swaps the tracks every viewer connection is sending for `new_stream`'s,
/// via `RTCRtpSender.replaceTrack` — no renegotiation, so viewers see only
/// a source change, not a reconnect. A sender is only replaced if
/// `new_stream` has a track of the same kind; a share that gains or loses
/// audio on the switch keeps its old audio sender state until the next full
/// re-share. Returns how many senders were swapped. Sender encoding params
/// (tier, audio preset) survive `replaceTrack`, so nothing needs
/// re-applying.
#[cfg(feature = "hydrate")]
pub(crate) async fn replace_outgoing_tracks(
    conn: &RoomSession,
    new_stream: &web_sys::MediaStream,
) -> usize {
    use wasm_bindgen::JsCast;
    use wasm_bindgen_futures::JsFuture;

    let (new_video, new_audio) = video_and_audio_tracks(new_stream);

    let mut replacements: Vec<(web_sys::RtcRtpSender, web_sys::MediaStreamTrack)> = Vec::new();
    for pc in conn.outgoing.borrow().values() {
        for entry in pc.get_senders().iter() {
            let sender: web_sys::RtcRtpSender = entry.unchecked_into();
            let Some(kind) = sender.track().map(|t| t.kind()) else {
                continue;
            };
            let replacement = match kind.as_str() {
                "video" => new_video.clone(),
                "audio" => new_audio.clone(),
                _ => None,
            };
            if let Some(track) = replacement {
                replacements.push((sender, track));
            }
        }
    }

    let mut swapped = 0;
    for (sender, track) in replacements {
        if JsFuture::from(sender.replace_track(Some(&track)))
            .await
            .is_ok()
        {
            swapped += 1;
        }
    }
    swapped
}

#[cfg(feature = "hydrate")]
pub(crate) fn stop_sharing(
    conn: &RoomSession,
    set_is_sharing: WriteSignal<bool>,
    own_preview_hidden: RwSignal<bool>,
    expanded: RwSignal<Option<String>>,
    my_peer_id: ReadSignal<Option<String>>,
) {
    use leptos::task::spawn_local;
    use wasm_bindgen::JsCast;

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
    if let Some(peer_id) = my_peer_id.get_untracked() {
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
    // Each viewer's Auto poll (if any) would otherwise keep firing against
    // a connection that's already closed.
    for viewer_peer_id in conn
        .quality_auto_intervals
        .borrow()
        .keys()
        .cloned()
        .collect::<Vec<_>>()
    {
        super::quality::stop_auto_polling(conn, &viewer_peer_id);
    }
    if let Some(ws) = conn.ws.borrow().as_ref() {
        ws.send(&screen_share_protocol::ClientMessage::StopShare);
    }
    set_is_sharing.set(false);
    own_preview_hidden.set(false);
    crate::infra::webrtc::notify_desktop_sharing_changed(false);
    // The server only sends `PeerStoppedSharing` to the other members, so
    // nobody else collapses the focus for us — without this the grid would
    // stay stuck in `grid--focused` with an empty card.
    expanded.update(|current| {
        if *current == my_peer_id.get_untracked() {
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
        set_status.set("Selecione a nova tela para compartilhar...".to_string());

        spawn_local(async move {
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
                if let Some(document) = web_sys::window().and_then(|w| w.document()) {
                    if let Some(video) =
                        document.get_element_by_id(&format!("video-self-{peer_id}"))
                    {
                        let video: web_sys::HtmlVideoElement = video.unchecked_into();
                        video.set_src_object(Some(&new_stream));
                        video.set_muted(true);
                        let _ = video.play();
                    }
                }
            }

            // Stop the tracks of the stream we just switched away from and
            // drop it, mirroring `stop_sharing`'s indicator-release dance,
            // then hold onto the new one.
            if let Some(old_stream) = conn.local_stream.borrow_mut().replace(new_stream.clone()) {
                for entry in old_stream.get_tracks().iter() {
                    let track: web_sys::MediaStreamTrack = entry.unchecked_into();
                    track.stop();
                    old_stream.remove_track(&track);
                }
            }

            attach_native_stop_listener(
                &new_stream,
                conn.clone(),
                set_is_sharing,
                own_preview_hidden,
                expanded,
                my_peer_id,
            );
            super::audio::set_shared_audio_muted(&conn, audio_muted.get_untracked());
            super::video_mode::apply_video_mode_to_all(&conn, video_mode.get_untracked()).await;
            set_status.set("Conectado.".to_string());
        });
    }
}

#[cfg(all(test, target_arch = "wasm32", feature = "hydrate"))]
#[path = "media_wasm_tests.rs"]
mod wasm_tests;
