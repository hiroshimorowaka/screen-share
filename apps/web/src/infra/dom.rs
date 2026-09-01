//! Browser event-listener / timer registrations that are torn down when
//! the current reactive owner (the component that called them) is
//! disposed.
//!
//! Replaces the `add_event_listener(...) + Closure::forget()` and
//! `set_interval(...) + Closure::forget()` pattern, which leaked past the
//! component: after `RoomPage` unmounted, a later `mousemove` / media-query
//! change reached an already-disposed reactive value and panicked, and the
//! self-ping timer kept calling `send` on a closed socket.

use leptos::prelude::on_cleanup;
use send_wrapper::SendWrapper;
use wasm_bindgen::prelude::Closure;
use wasm_bindgen::JsCast;
use web_sys::EventTarget;

/// Adds `callback` as a listener for `event` on `target`, and removes it
/// (dropping the `Closure`) when the current owner is cleaned up.
///
/// `target` is anything that is an `EventTarget` — `window`, `document`, a
/// `MediaQueryList`, an element.
pub fn listen_until_cleanup(
    target: impl AsRef<EventTarget>,
    event: &'static str,
    callback: Closure<dyn FnMut()>,
) {
    let target: EventTarget = target.as_ref().clone();
    // `removeEventListener` matches the listener by identity, so add and
    // remove must pass the same `Function` object.
    let handler = callback
        .as_ref()
        .unchecked_ref::<js_sys::Function>()
        .clone();
    let _ = target.add_event_listener_with_callback(event, &handler);

    // `on_cleanup` is `Send + Sync`-bound; these JS handles are `!Send`.
    // `SendWrapper` is sound on single-threaded wasm — it panics if ever
    // touched from another thread, which never happens here.
    let owned = SendWrapper::new((target, handler, callback));
    on_cleanup(move || {
        let (target, handler, callback) = owned.take();
        let _ = target.remove_event_listener_with_callback(event, &handler);
        drop(callback);
    });
}

/// Registers `callback` on a `setInterval` of `interval_ms`, and clears
/// the interval (dropping the `Closure`) when the current owner is cleaned
/// up. No-op if there is no `window` or the interval can't be created.
pub fn interval_until_cleanup(callback: Closure<dyn FnMut()>, interval_ms: i32) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let Ok(interval_id) = window.set_interval_with_callback_and_timeout_and_arguments_0(
        callback.as_ref().unchecked_ref(),
        interval_ms,
    ) else {
        return;
    };

    let owned = SendWrapper::new((window, callback));
    on_cleanup(move || {
        let (window, callback) = owned.take();
        window.clear_interval_with_handle(interval_id);
        drop(callback);
    });
}

#[cfg(all(test, target_arch = "wasm32", feature = "hydrate"))]
#[path = "dom_wasm_tests.rs"]
mod wasm_tests;
