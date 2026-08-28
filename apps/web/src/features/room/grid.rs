use leptos::prelude::*;

use crate::session::RoomMember;

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
            if let Ok(id) = window.set_timeout_with_callback_and_timeout_and_arguments_0(
                hide.as_ref().unchecked_ref(),
                HIDE_AFTER_MS,
            ) {
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
        Closure::<dyn FnMut()>::new(show_and_schedule_hide)
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

    let on_resize = Closure::<dyn FnMut()>::new(schedule_recompute);
    if let Some(window) = web_sys::window() {
        let _ =
            window.add_event_listener_with_callback("resize", on_resize.as_ref().unchecked_ref());
    }
    on_resize.forget();
}

/// Picks how many columns get each tile closest to a 16:9 rectangle, given
/// the container's actual pixel size. Checks every candidate directly
/// (there are at most `MAX_MEMBERS` of them) instead of a closed-form
/// approximation — a single-shot formula can land tiles far from 16:9 for
/// odd member counts on a wide window (5 people never splits evenly into a
/// 16:9 tile, and the wrong column count stretched tiles into ~3:1
/// rectangles instead of something roughly square). Comparing aspect ratios
/// as a log-ratio treats "too wide" and "too tall" symmetrically, so it
/// doesn't systematically favor one over the other.
///
/// Only called from `recompute_adaptive_grid`, which is `hydrate`-only — an
/// `ssr`-only, non-test compile never calls this and would otherwise flag
/// it as dead code.
#[cfg_attr(not(any(feature = "hydrate", test)), allow(dead_code))]
fn best_column_count(visible: usize, width: f64, height: f64) -> usize {
    const TILE_ASPECT: f64 = 16.0 / 9.0;

    (1..=visible)
        .map(|cols| {
            let rows = visible.div_ceil(cols);
            let tile_aspect = (width / cols as f64) / (height / rows as f64);
            (cols, (tile_aspect / TILE_ASPECT).ln().abs())
        })
        .min_by(|(_, a), (_, b)| a.total_cmp(b))
        .map_or(1, |(cols, _)| cols)
}

#[cfg(feature = "hydrate")]
fn recompute_adaptive_grid(expanded: RwSignal<Option<String>>) {
    use wasm_bindgen::JsCast;

    let Some(document) = web_sys::window().and_then(|w| w.document()) else {
        return;
    };
    let Some(grid) = document.get_element_by_id("member-grid") else {
        return;
    };
    let grid: web_sys::HtmlElement = grid.unchecked_into();
    let style = grid.style();

    if expanded.get_untracked().is_some() {
        // `.grid--focused` already defines its own grid-template via CSS —
        // clear any inline value left by normal mode, or the inline value
        // (which takes priority over the class rule) locks up focus mode's
        // layout. Same for the per-card `grid-column`/`grid-row` normal mode
        // sets to center a sparse last row — left in place, their higher
        // specificity than any class rule would misplace the filmstrip
        // cards too.
        let _ = style.remove_property("grid-template-columns");
        let _ = style.remove_property("grid-template-rows");
        if let Ok(cards) = grid.query_selector_all(".card") {
            for i in 0..cards.length() {
                let Some(card) = cards
                    .item(i)
                    .and_then(|node| node.dyn_into::<web_sys::HtmlElement>().ok())
                else {
                    continue;
                };
                let _ = card.style().remove_property("grid-column");
                let _ = card.style().remove_property("grid-row");
            }
        }
        return;
    }

    let Ok(list) = grid.query_selector_all(".card:not(.hidden)") else {
        return;
    };
    let visible = list.length() as usize;
    if visible == 0 {
        return;
    }

    let width = grid.client_width() as f64;
    let height = grid.client_height() as f64;
    if width <= 0.0 || height <= 0.0 {
        return;
    }

    let cols = best_column_count(visible, width, height);
    let rows = visible.div_ceil(cols);

    // Each real column is two grid tracks wide, so a sparse last row (fewer
    // cards than `cols`) can be centered — like Discord's own call grid —
    // by giving just that row's cards a start offset in half-column units,
    // instead of leaving them left-aligned by default auto-placement. Every
    // card gets an explicit `grid-column`/`grid-row` rather than relying on
    // auto-placement for the rest: mixing an explicit offset for some cards
    // with auto-placed siblings isn't guaranteed to land them in the same
    // row.
    let _ = style.set_property(
        "grid-template-columns",
        &format!("repeat({}, minmax(0, 1fr))", cols * 2),
    );
    let _ = style.set_property(
        "grid-template-rows",
        &format!("repeat({rows}, minmax(0, 1fr))"),
    );

    let remainder = visible % cols;
    let last_row_start = visible - remainder;
    let center_offset = cols - remainder;

    for i in 0..visible {
        let Some(card) = list
            .item(i as u32)
            .and_then(|node| node.dyn_into::<web_sys::HtmlElement>().ok())
        else {
            continue;
        };

        let (row, slot_in_row, row_len) = if remainder > 0 && i >= last_row_start {
            (rows, i - last_row_start, remainder)
        } else {
            (i / cols + 1, i % cols, cols)
        };
        let offset = if row_len < cols { center_offset } else { 0 };
        let start = 1 + offset + slot_in_row * 2;

        let card_style = card.style();
        let _ = card_style.set_property("grid-column", &format!("{start} / span 2"));
        let _ = card_style.set_property("grid-row", &row.to_string());
    }
}

#[cfg(test)]
#[path = "grid_tests.rs"]
mod tests;
