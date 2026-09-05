//! Browser event-listener / timer registrations that are torn down when
//! the current reactive owner (the component that called them) is
//! disposed.
//!
//! Replaces the `add_event_listener(...) + Closure::forget()` and
//! `set_interval(...) + Closure::forget()` pattern, which leaked past the
//! component: after `RoomPage` unmounted, a later `mousemove` / media-query
//! change reached an already-disposed reactive value and panicked, and the
//! self-ping timer kept calling `send` on a closed socket.

use leptos::prelude::*;
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

/// Delay before a dismissible error status reverts to its replacement —
/// long enough to read a full sentence-length error, short enough that the
/// panel doesn't look permanently broken (the bug this fixes: a validation
/// error like a too-long nick used to sit on screen forever).
const ERROR_DISMISS_MS: i32 = 6000;

/// Watches `status`; whenever it becomes an error status that
/// [`screen_share_domain::status::is_dismissible_error`] says should clear
/// itself, reverts it to `replacement` after [`ERROR_DISMISS_MS`].
/// Re-armed, not stacked, on every change: a second error restarts the
/// clock, and a non-dismissible status (a retry succeeding, a new attempt
/// starting) cancels the pending revert outright. Guards against a stale
/// timer clobbering a message that already changed while it was pending.
pub fn auto_dismiss_error(
    status: ReadSignal<String>,
    set_status: WriteSignal<String>,
    replacement: &'static str,
) {
    use std::cell::Cell;
    use std::rc::Rc;

    use screen_share_domain::status::is_dismissible_error;

    let Some(window) = web_sys::window() else {
        return;
    };
    let timeout_id: Rc<Cell<Option<i32>>> = Rc::new(Cell::new(None));

    Effect::new(move |_| {
        let current = status.get();
        if let Some(id) = timeout_id.take() {
            window.clear_timeout_with_handle(id);
        }
        if !is_dismissible_error(&current) {
            return;
        }
        let revert = Closure::once_into_js(move || {
            if status.get_untracked() == current {
                set_status.set(replacement.to_string());
            }
        });
        if let Ok(id) = window.set_timeout_with_callback_and_timeout_and_arguments_0(
            revert.as_ref().unchecked_ref(),
            ERROR_DISMISS_MS,
        ) {
            timeout_id.set(Some(id));
        }
    });
}

#[cfg(all(test, target_arch = "wasm32", feature = "hydrate"))]
#[path = "wasm_tests.rs"]
mod wasm_tests;
