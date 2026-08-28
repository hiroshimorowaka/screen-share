use leptos::prelude::*;

#[cfg(not(feature = "hydrate"))]
pub(super) fn invite_click_handler(
    _room_code: String,
    _invite_copied: RwSignal<bool>,
) -> impl Fn(leptos::ev::MouseEvent) + Clone + 'static {
    move |_| {}
}

/// How long `invite_copied` stays on to visually confirm the copy — the
/// Clipboard API doesn't notify on its own.
#[cfg(feature = "hydrate")]
const COPIED_INDICATOR_MS: i32 = 2000;

/// The full invite link for a room, built from the current page's origin.
/// Shared by the invite button and the desktop tray's quick-share flow,
/// which hands this same link to the Electron shell to copy on the
/// sharer's behalf.
#[cfg(feature = "hydrate")]
pub(super) fn build_invite_link(room_code: &str) -> Option<String> {
    let window = web_sys::window()?;
    let origin = window.location().origin().ok()?;
    Some(format!("{origin}/r/{room_code}"))
}

#[cfg(feature = "hydrate")]
pub(super) fn invite_click_handler(
    room_code: String,
    invite_copied: RwSignal<bool>,
) -> impl Fn(leptos::ev::MouseEvent) + Clone + 'static {
    use leptos::task::spawn_local;
    use wasm_bindgen::JsCast;
    use wasm_bindgen_futures::JsFuture;

    move |_| {
        let Some(window) = web_sys::window() else {
            return;
        };
        let Some(link) = build_invite_link(&room_code) else {
            return;
        };
        let promise = window.navigator().clipboard().write_text(&link);

        spawn_local(async move {
            if JsFuture::from(promise).await.is_err() {
                return;
            }
            invite_copied.set(true);
            let Some(window) = web_sys::window() else {
                return;
            };
            let reset =
                wasm_bindgen::prelude::Closure::once_into_js(move || invite_copied.set(false));
            let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(
                reset.as_ref().unchecked_ref(),
                COPIED_INDICATOR_MS,
            );
        });
    }
}
