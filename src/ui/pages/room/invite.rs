use leptos::prelude::*;

#[cfg(not(feature = "hydrate"))]
pub(super) fn invite_click_handler(_room_code: String, _invite_copied: RwSignal<bool>) -> impl Fn(leptos::ev::MouseEvent) + Clone + 'static {
    move |_| {}
}

/// `invite_copied` stays on for 2s only to visually confirm the copy — the
/// Clipboard API doesn't notify on its own.
#[cfg(feature = "hydrate")]
pub(super) fn invite_click_handler(room_code: String, invite_copied: RwSignal<bool>) -> impl Fn(leptos::ev::MouseEvent) + Clone + 'static {
    use leptos::task::spawn_local;
    use wasm_bindgen::JsCast;
    use wasm_bindgen_futures::JsFuture;

    move |_| {
        let Some(window) = web_sys::window() else { return };
        let origin = window.location().origin().unwrap_or_default();
        let link = format!("{origin}/r/{room_code}");
        let promise = window.navigator().clipboard().write_text(&link);

        spawn_local(async move {
            if JsFuture::from(promise).await.is_err() {
                return;
            }
            invite_copied.set(true);
            let Some(window) = web_sys::window() else { return };
            let reset = wasm_bindgen::prelude::Closure::once_into_js(move || invite_copied.set(false));
            let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(reset.as_ref().unchecked_ref(), 2000);
        });
    }
}
