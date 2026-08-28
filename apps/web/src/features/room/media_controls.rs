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
    let Some(document) = web_sys::window().and_then(|w| w.document()) else {
        return;
    };

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
pub(super) fn exit_fullscreen_if_active() -> bool {
    false
}

/// Used by the card's own click-to-expand handler: clicking anywhere on a
/// card that's currently fullscreen should back out of fullscreen and leave
/// the expanded/normal state exactly as it was before — not toggle it, which
/// used to happen invisibly (fullscreen hides the layout difference between
/// the two) and left the wrong mode showing once the user exited fullscreen
/// by other means (Esc, browser controls). Returns whether fullscreen was
/// actually active, so the caller can skip its own click behavior.
#[cfg(feature = "hydrate")]
pub(super) fn exit_fullscreen_if_active() -> bool {
    let Some(document) = web_sys::window().and_then(|w| w.document()) else {
        return false;
    };
    if document.fullscreen_element().is_none() {
        return false;
    }
    document.exit_fullscreen();
    true
}

/// Used when a watched peer stops sharing or leaves the room: if their card
/// (identified by the `card-{peer_id}` id set in `member_card.rs`) is the
/// one currently in fullscreen, back out of fullscreen instead of leaving
/// the browser stuck there — the fullscreen API has no idea the video
/// feeding it just disappeared, so nothing else would exit it automatically.
#[cfg(feature = "hydrate")]
pub(crate) fn exit_fullscreen_if_showing(peer_id: &str) -> bool {
    let Some(document) = web_sys::window().and_then(|w| w.document()) else {
        return false;
    };
    let Some(fullscreen_element) = document.fullscreen_element() else {
        return false;
    };
    if fullscreen_element.id() != format!("card-{peer_id}") {
        return false;
    }
    document.exit_fullscreen();
    true
}

#[cfg(not(feature = "hydrate"))]
pub(super) fn blur_active_element() {}

/// Drops focus from whatever element currently holds it. The quality menu
/// stays open while it has `:focus-within`, so after a click on one of its
/// options the popup would otherwise linger (focus stuck on the clicked
/// button) until the user clicks elsewhere — blurring lets it close as
/// soon as the pointer leaves, like a normal menu.
#[cfg(feature = "hydrate")]
pub(super) fn blur_active_element() {
    use wasm_bindgen::JsCast;

    let Some(active) = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.active_element())
    else {
        return;
    };
    if let Some(el) = active.dyn_ref::<web_sys::HtmlElement>() {
        let _ = el.blur();
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

    let Some(document) = web_sys::window().and_then(|w| w.document()) else {
        return;
    };

    if document.picture_in_picture_element().is_some() {
        let promise = document.exit_picture_in_picture();
        spawn_local(async move {
            let _ = JsFuture::from(promise).await;
        });
        return;
    }

    let Some(video) = document.get_element_by_id(&slot.element_id(peer_id)) else {
        return;
    };
    let video: web_sys::HtmlVideoElement = video.unchecked_into();
    let promise = video.request_picture_in_picture();
    spawn_local(async move {
        let _ = JsFuture::from(promise).await;
    });
}

#[cfg(not(feature = "hydrate"))]
pub(super) fn set_volume(_slot: VideoSlot, _peer_id: &str, _volume: f64) {}

/// `volume` is clamped to `[0, 1]` — `HtmlMediaElement::set_volume` panics
/// (throws) outside that range.
#[cfg(feature = "hydrate")]
pub(super) fn set_volume(slot: VideoSlot, peer_id: &str, volume: f64) {
    use wasm_bindgen::JsCast;

    let Some(document) = web_sys::window().and_then(|w| w.document()) else {
        return;
    };
    let Some(video) = document.get_element_by_id(&slot.element_id(peer_id)) else {
        return;
    };
    let video: web_sys::HtmlVideoElement = video.unchecked_into();
    video.set_volume(volume.clamp(0.0, 1.0));
}

#[cfg(not(feature = "hydrate"))]
pub(super) fn set_muted(_slot: VideoSlot, _peer_id: &str, _muted: bool) {}

#[cfg(feature = "hydrate")]
pub(super) fn set_muted(slot: VideoSlot, peer_id: &str, muted: bool) {
    use wasm_bindgen::JsCast;

    let Some(document) = web_sys::window().and_then(|w| w.document()) else {
        return;
    };
    let Some(video) = document.get_element_by_id(&slot.element_id(peer_id)) else {
        return;
    };
    let video: web_sys::HtmlVideoElement = video.unchecked_into();
    video.set_muted(muted);
}

#[cfg(not(feature = "hydrate"))]
pub(super) fn setup_fullscreen_autohide_controls() {}

/// Marks the fullscreen `.card` with `card--controls-idle` (see `card.css`)
/// after a few seconds without mouse movement, and clears it again on the
/// next move — the same convention native video players use. Without this,
/// `.card:hover` (which normally reveals `.card__actions`) never turns
/// false in fullscreen, since the pointer can't leave a card that fills the
/// whole screen, so the stop-watching/exit-fullscreen buttons stayed
/// visible forever. Runs once for the whole page (there's only ever one
/// fullscreen element at a time), rather than per-card.
#[cfg(feature = "hydrate")]
pub(super) fn setup_fullscreen_autohide_controls() {
    use std::cell::Cell;
    use std::rc::Rc;

    use wasm_bindgen::prelude::Closure;
    use wasm_bindgen::JsCast;

    /// How long the pointer can sit still before the controls fade — long
    /// enough to read a label, short enough not to linger over the video.
    const HIDE_AFTER_MS: i32 = 3000;
    const IDLE_CLASS: &str = "card--controls-idle";

    let Some(window) = web_sys::window() else {
        return;
    };
    let Some(document) = window.document() else {
        return;
    };
    let timeout_id: Rc<Cell<Option<i32>>> = Rc::new(Cell::new(None));

    let cancel_pending = {
        let window = window.clone();
        let timeout_id = timeout_id.clone();
        move || {
            if let Some(id) = timeout_id.take() {
                window.clear_timeout_with_handle(id);
            }
        }
    };

    let clear_idle = {
        let document = document.clone();
        move || {
            if let Some(el) = document.fullscreen_element() {
                let _ = el.class_list().remove_1(IDLE_CLASS);
            }
        }
    };

    let schedule_idle = {
        let window = window.clone();
        let document = document.clone();
        let timeout_id = timeout_id.clone();
        let cancel_pending = cancel_pending.clone();
        move || {
            cancel_pending();
            let document = document.clone();
            let mark_idle = Closure::once_into_js(move || {
                if let Some(el) = document.fullscreen_element() {
                    let _ = el.class_list().add_1(IDLE_CLASS);
                }
            });
            if let Ok(id) = window.set_timeout_with_callback_and_timeout_and_arguments_0(
                mark_idle.as_ref().unchecked_ref(),
                HIDE_AFTER_MS,
            ) {
                timeout_id.set(Some(id));
            }
        }
    };

    let on_mousemove = {
        let clear_idle = clear_idle.clone();
        let schedule_idle = schedule_idle.clone();
        Closure::<dyn FnMut()>::new(move || {
            clear_idle();
            schedule_idle();
        })
    };
    let _ = document
        .add_event_listener_with_callback("mousemove", on_mousemove.as_ref().unchecked_ref());
    on_mousemove.forget();

    let on_fullscreenchange = {
        let document = document.clone();
        Closure::<dyn FnMut()>::new(move || {
            cancel_pending();
            clear_idle();
            if document.fullscreen_element().is_some() {
                schedule_idle();
            }
        })
    };
    let _ = document.add_event_listener_with_callback(
        "fullscreenchange",
        on_fullscreenchange.as_ref().unchecked_ref(),
    );
    on_fullscreenchange.forget();
}
