use leptos::prelude::*;

use super::RoomMember;

#[cfg(not(feature = "hydrate"))]
pub(super) fn setup_auto_hide_controls(
    _controls_visible: RwSignal<bool>,
) -> (impl Fn() + Clone + 'static, impl Fn() + Clone + 'static) {
    (|| {}, || {})
}

#[cfg(feature = "hydrate")]
pub(super) fn setup_auto_hide_controls(
    controls_visible: RwSignal<bool>,
) -> (impl Fn() + Clone + 'static, impl Fn() + Clone + 'static) {
    use std::cell::Cell;
    use std::rc::Rc;

    use wasm_bindgen::prelude::Closure;
    use wasm_bindgen::JsCast;

    const HIDE_AFTER_MS: i32 = 3000;

    let window = web_sys::window().expect("the hydrate function runs inside a real browser");
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

    let schedule_hide = {
        let window = window.clone();
        let timeout_id = timeout_id.clone();
        let cancel_pending = cancel_pending.clone();
        move || {
            cancel_pending();
            let hide = Closure::once_into_js(move || controls_visible.set(false));
            if let Ok(id) = window
                .set_timeout_with_callback_and_timeout_and_arguments_0(hide.as_ref().unchecked_ref(), HIDE_AFTER_MS)
            {
                timeout_id.set(Some(id));
            }
        }
    };

    let show_and_schedule_hide = {
        let schedule_hide = schedule_hide.clone();
        move || {
            controls_visible.set(true);
            schedule_hide();
        }
    };

    let on_move = {
        let show_and_schedule_hide = show_and_schedule_hide.clone();
        Closure::<dyn FnMut()>::new(move || show_and_schedule_hide())
    };
    let _ = window.add_event_listener_with_callback("mousemove", on_move.as_ref().unchecked_ref());
    on_move.forget();

    show_and_schedule_hide();

    (cancel_pending, schedule_hide)
}

#[cfg(not(feature = "hydrate"))]
pub(super) fn setup_adaptive_grid(
    _members: ReadSignal<Vec<RoomMember>>,
    _hide_idle: RwSignal<bool>,
    _own_preview_hidden: RwSignal<bool>,
    _is_sharing: ReadSignal<bool>,
    _expanded: RwSignal<Option<String>>,
) {
}

/// Same idea as Discord: how many columns/rows the grid uses depends on how
/// many cards are visible *and* the container's own aspect ratio — 1 person
/// fills the whole screen, 2 sit side by side (or stacked, if the window is
/// taller than it is wide), and so on, unlike a fixed `grid-template-columns`
/// that would leave tiny cards floating in a giant container when there are
/// few members. Since this calculation depends on the container's actual
/// pixel size (not just the count), it can't be done with CSS alone — hence
/// `grid-template-columns/rows` is written via inline `style` after
/// measuring the container. `#[cfg(feature = "hydrate")]` already filters
/// this, so the function only ever runs with a real browser available.
#[cfg(feature = "hydrate")]
pub(super) fn setup_adaptive_grid(
    members: ReadSignal<Vec<RoomMember>>,
    hide_idle: RwSignal<bool>,
    own_preview_hidden: RwSignal<bool>,
    is_sharing: ReadSignal<bool>,
    expanded: RwSignal<Option<String>>,
) {
    use wasm_bindgen::prelude::Closure;
    use wasm_bindgen::JsCast;

    let schedule_recompute = move || {
        let raf_callback = Closure::once_into_js(move || recompute_adaptive_grid(expanded));
        if let Some(window) = web_sys::window() {
            let _ = window.request_animation_frame(raf_callback.as_ref().unchecked_ref());
        }
    };

    // React to any signal that changes how many cards are visible (joining/
    // leaving the room, hiding idle members, hiding one's own preview,
    // starting/stopping sharing, entering/exiting focus mode). The actual
    // recount happens on the next frame (`schedule_recompute`), after the
    // DOM has already reflected the new `hidden` classes.
    Effect::new(move |_| {
        members.track();
        hide_idle.track();
        own_preview_hidden.track();
        is_sharing.track();
        expanded.track();
        schedule_recompute();
    });

    let on_resize = Closure::<dyn FnMut()>::new(move || schedule_recompute());
    if let Some(window) = web_sys::window() {
        let _ = window.add_event_listener_with_callback("resize", on_resize.as_ref().unchecked_ref());
    }
    on_resize.forget();
}

#[cfg(feature = "hydrate")]
fn recompute_adaptive_grid(expanded: RwSignal<Option<String>>) {
    use wasm_bindgen::JsCast;

    let Some(document) = web_sys::window().and_then(|w| w.document()) else { return };
    let Some(grid) = document.get_element_by_id("member-grid") else { return };
    let grid: web_sys::HtmlElement = grid.unchecked_into();
    let style = grid.style();

    if expanded.get_untracked().is_some() {
        // `.grid--focused` already defines its own grid-template via CSS —
        // clear any inline value left by normal mode, or the inline value
        // (which takes priority over the class rule) locks up focus mode's
        // layout.
        let _ = style.remove_property("grid-template-columns");
        let _ = style.remove_property("grid-template-rows");
        return;
    }

    let Ok(list) = grid.query_selector_all(".card:not(.hidden)") else { return };
    let visible = list.length() as usize;
    if visible == 0 {
        return;
    }

    let width = grid.client_width() as f64;
    let height = grid.client_height() as f64;
    if width <= 0.0 || height <= 0.0 {
        return;
    }

    // Classic video-call grid formula: pick the number of columns that gets
    // each cell as close as possible to a 16:9 rectangle, given the
    // container's actual aspect ratio — that's why 2 people sit side by
    // side in a wide window but stacked in a narrow one.
    const TILE_ASPECT: f64 = 16.0 / 9.0;
    let container_ratio = width / height;
    let raw_cols = ((visible as f64) * container_ratio / TILE_ASPECT).sqrt();
    let cols = (raw_cols.round().max(1.0) as usize).min(visible);
    let rows = visible.div_ceil(cols);

    let _ = style.set_property("grid-template-columns", &format!("repeat({cols}, minmax(0, 1fr))"));
    let _ = style.set_property("grid-template-rows", &format!("repeat({rows}, minmax(0, 1fr))"));
}
