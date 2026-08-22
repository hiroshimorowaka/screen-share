/// Which of a card's two fixed `<video>` elements to target — see the `id`
/// scheme in `member_card.rs` (`video-self-{peer_id}` vs `video-{peer_id}`).
#[derive(Clone, Copy)]
pub(super) enum VideoSlot {
    Own,
    Peer,
}

#[cfg(feature = "hydrate")]
impl VideoSlot {
    fn element_id(self, peer_id: &str) -> String {
        match self {
            VideoSlot::Own => format!("video-self-{peer_id}"),
            VideoSlot::Peer => format!("video-{peer_id}"),
        }
    }
}

#[cfg(not(feature = "hydrate"))]
pub(super) fn toggle_fullscreen(_slot: VideoSlot, _peer_id: &str) {}

/// Puts the `.card` (not the `<video>`) into fullscreen: if it were the
/// video, Chrome injects native play/pause/seek controls over it, which
/// makes no sense for a live broadcast.
#[cfg(feature = "hydrate")]
pub(super) fn toggle_fullscreen(slot: VideoSlot, peer_id: &str) {
    let Some(document) = web_sys::window().and_then(|w| w.document()) else { return };

    if document.fullscreen_element().is_some() {
        document.exit_fullscreen();
        return;
    }

    let video = document.get_element_by_id(&slot.element_id(peer_id));
    let card = video.and_then(|v| v.closest(".card").ok().flatten());
    if let Some(card) = card {
        let _ = card.request_fullscreen();
    }
}

#[cfg(not(feature = "hydrate"))]
pub(super) fn toggle_picture_in_picture(_slot: VideoSlot, _peer_id: &str) {}

/// Same `id` scheme as `toggle_fullscreen` uses to find the right video
/// among the fixed slots. Only one PiP window at a time is a browser
/// limitation.
#[cfg(feature = "hydrate")]
pub(super) fn toggle_picture_in_picture(slot: VideoSlot, peer_id: &str) {
    use leptos::task::spawn_local;
    use wasm_bindgen::JsCast;
    use wasm_bindgen_futures::JsFuture;

    let Some(document) = web_sys::window().and_then(|w| w.document()) else { return };

    if document.picture_in_picture_element().is_some() {
        let promise = document.exit_picture_in_picture();
        spawn_local(async move {
            let _ = JsFuture::from(promise).await;
        });
        return;
    }

    let Some(video) = document.get_element_by_id(&slot.element_id(peer_id)) else { return };
    let video: web_sys::HtmlVideoElement = video.unchecked_into();
    let promise = video.request_picture_in_picture();
    spawn_local(async move {
        let _ = JsFuture::from(promise).await;
    });
}
