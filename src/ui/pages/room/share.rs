use leptos::prelude::*;

use super::connection::RoomConnection;

#[cfg(not(feature = "hydrate"))]
pub(super) fn share_supported() -> bool {
    true
}

#[cfg(feature = "hydrate")]
pub(super) fn share_supported() -> bool {
    crate::ui::client::webrtc::is_display_media_supported()
}

#[cfg(not(feature = "hydrate"))]
pub(super) fn share_toggle_handler(
    _conn: RoomConnection,
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
pub(super) fn share_toggle_handler(
    conn: RoomConnection,
    is_sharing: ReadSignal<bool>,
    set_is_sharing: WriteSignal<bool>,
    own_preview_hidden: RwSignal<bool>,
    set_status: WriteSignal<String>,
    my_peer_id: ReadSignal<Option<String>>,
    expanded: RwSignal<Option<String>>,
) -> impl Fn(leptos::ev::MouseEvent) + Clone + 'static {
    use leptos::task::spawn_local;
    use wasm_bindgen::JsCast;
    use web_sys::MediaStreamTrack;

    use crate::signaling::protocol::ClientMessage;
    use crate::ui::client::webrtc::capture_display;

    move |_| {
        if is_sharing.get_untracked() {
            stop_sharing(
                &conn,
                set_is_sharing,
                own_preview_hidden,
                expanded,
                my_peer_id,
            );
            return;
        }

        let conn = conn.clone();
        let my_peer_id_value = my_peer_id.get_untracked();
        set_status.set("Selecione a tela para compartilhar...".to_string());

        spawn_local(async move {
            let stream = match capture_display().await {
                Ok(stream) => stream,
                Err(_) => {
                    set_status.set("Conectado.".to_string());
                    return;
                }
            };

            if let Some(peer_id) = my_peer_id_value.as_deref() {
                if let Some(document) = web_sys::window().and_then(|w| w.document()) {
                    if let Some(video) =
                        document.get_element_by_id(&format!("video-self-{peer_id}"))
                    {
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
            set_is_sharing.set(true);

            // The browser's own native "Stop sharing" button fires `onended`
            // directly on the track, without going through our `toggle_share`.
            if let Ok(track) = stream.get_tracks().get(0).dyn_into::<MediaStreamTrack>() {
                let conn_for_end = conn.clone();
                let onended = wasm_bindgen::prelude::Closure::<dyn FnMut()>::new(move || {
                    stop_sharing(
                        &conn_for_end,
                        set_is_sharing,
                        own_preview_hidden,
                        expanded,
                        my_peer_id,
                    );
                });
                track.set_onended(Some(onended.as_ref().unchecked_ref()));
                onended.forget();
            }

            if let Some(ws) = conn.ws.borrow().as_ref() {
                ws.send(&ClientMessage::StartShare);
            }

            // Store the same `stream` handle used above, not a `.clone()` of
            // it — cloning here and letting this original drop at the end of
            // the block was enough, on its own, to keep Chrome's native
            // "sharing" indicator from ever releasing later, even though the
            // clone kept working fine for playback and `stop()`.
            *conn.local_stream.borrow_mut() = Some(stream);
        });
    }
}

#[cfg(feature = "hydrate")]
pub(super) fn stop_sharing(
    conn: &RoomConnection,
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
        let _ = crate::ui::client::webrtc::stop_desktop_audio_loopback().await;
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
    if let Some(ws) = conn.ws.borrow().as_ref() {
        ws.send(&crate::signaling::protocol::ClientMessage::StopShare);
    }
    set_is_sharing.set(false);
    own_preview_hidden.set(false);
    // The server only sends `PeerStoppedSharing` to the other members, so
    // nobody else collapses the focus for us — without this the grid would
    // stay stuck in `grid--focused` with an empty card.
    expanded.update(|current| {
        if *current == my_peer_id.get_untracked() {
            *current = None;
        }
    });
}
