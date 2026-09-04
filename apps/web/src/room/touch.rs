//! Whether this is a touch device — one reactive bit, kept in sync with a
//! `matchMedia` query. Almost all of the mobile adaptation is pure CSS
//! (`@media (hover: none) and (pointer: coarse)`); this signal exists only
//! for the handful of Leptos handlers that genuinely have to branch
//! (a tap on the focused video toggling the chrome, and the control-bar
//! auto-hide that behaves differently with no pointer to track).

use leptos::prelude::*;

/// The media query that means "no hover, coarse pointer" — a phone or a
/// tablet held in the hand, not a laptop with a trackpad.
#[cfg(feature = "hydrate")]
const TOUCH_QUERY: &str = "(hover: none) and (pointer: coarse)";

#[cfg(not(feature = "hydrate"))]
pub(crate) fn setup_touch_signal(_set_is_touch: WriteSignal<bool>) {}

/// Sets `set_is_touch` from `TOUCH_QUERY` now and on every change (a tablet
/// paired with a mouse, or DevTools device emulation, can flip it at
/// runtime). The listener is removed when `RoomPage` is disposed — the
/// signal it writes belongs to `RoomPage`, so a `change` firing after that
/// would hit an already-disposed value.
#[cfg(feature = "hydrate")]
pub(crate) fn setup_touch_signal(set_is_touch: WriteSignal<bool>) {
    use wasm_bindgen::prelude::Closure;

    let Some(window) = web_sys::window() else {
        return;
    };
    let Ok(Some(query)) = window.match_media(TOUCH_QUERY) else {
        return;
    };

    set_is_touch.set(query.matches());

    let on_change = Closure::<dyn FnMut()>::new({
        let query = query.clone();
        move || set_is_touch.set(query.matches())
    });
    crate::client::dom::listen_until_cleanup(&query, "change", on_change);
}
