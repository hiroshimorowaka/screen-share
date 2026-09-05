//! The two stateful widgets that live in a member card's action bar,
//! pulled out of `MemberCard` so its body stays about the card shell.
//! Both are `!Send` in practice (their handlers capture the room
//! `RoomSession`) so they are only ever called directly, never through
//! `<For>`.

use leptos::prelude::*;

use super::{quality_label, QUALITY_LEVELS};
use crate::components::ui::icons::{icon_bars, icon_chevron_down, icon_volume, icon_volume_off};
use crate::room::media_controls::{
    blur_active_element, event_target_already_focused, set_muted, set_volume, VideoSlot,
};
use screen_share_protocol::QualityLevel;

/// The per-stream video-quality dropdown. A custom menu rather than a
/// native `<select>` — the browser's option list can't be themed to match
/// the card. Desktop opens it on hover (see `.quality-menu` in
/// card-widgets.css); touch has no hover, so it opens on tap / keyboard
/// focus and this component blurs the trigger to close it again.
#[component]
pub(super) fn QualityMenu<F>(
    /// Hidden unless this viewer is currently watching this stream.
    #[prop(into)]
    hidden: Signal<bool>,
    is_touch: ReadSignal<bool>,
    /// The level shown selected in the trigger and the popup.
    #[prop(into)]
    current: Signal<QualityLevel>,
    /// Applies a chosen level to this (sharer, viewer) connection.
    on_select: F,
) -> impl IntoView
where
    F: Fn(QualityLevel) + Clone + 'static,
{
    // Set from the trigger's `mousedown` (before the browser's own
    // focus-on-mousedown applies), read from its `click` right after — see
    // `event_target_already_focused`.
    let quality_trigger_was_open = RwSignal::new(false);
    view! {
        <div
            class="quality-menu"
            class:hidden=move || hidden.get()
            on:click=move |ev: leptos::ev::MouseEvent| ev.stop_propagation()
        >
            <button
                type="button"
                class="quality-menu__trigger"
                title="Qualidade do vídeo"
                aria-label="Qualidade do vídeo"
                on:mousedown=move |ev: leptos::ev::MouseEvent| {
                    quality_trigger_was_open.set(event_target_already_focused(&ev));
                }
                on:click=move |_| {
                    // Touch only: a tap already opens the menu by focusing
                    // the trigger (see the CSS `:focus-within` rule) —
                    // clicking the same, already-focused button again
                    // doesn't blur it on its own, so do it explicitly to
                    // close.
                    if is_touch.get_untracked() && quality_trigger_was_open.get() {
                        blur_active_element();
                    }
                }
            >
                {icon_bars()}
                <span class="quality-menu__current">{move || quality_label(current.get())}</span>
                {icon_chevron_down()}
            </button>
            <div class="quality-menu__popup">
                {QUALITY_LEVELS
                    .into_iter()
                    .map(move |level| {
                        let on_select = on_select.clone();
                        view! {
                            <button
                                type="button"
                                class="quality-menu__option"
                                class:quality-menu__option--active=move || current.get() == level
                                on:click=move |ev: leptos::ev::MouseEvent| {
                                    ev.stop_propagation();
                                    on_select(level);
                                    blur_active_element();
                                }
                            >
                                {quality_label(level)}
                            </button>
                        }
                    })
                    .collect::<Vec<_>>()}
            </div>
        </div>
    }
}

/// The per-stream volume control: one small floating popup (see
/// `.volume-control` in card-widgets.css), unchanged between mouse and
/// touch — a vertical slider anchored above the button. On a mouse the
/// popup opens on hover and the button is an instant mute toggle. Touch
/// has no hover, so there the *first* tap only opens the popup (the
/// button focuses on tap, and `:focus-within` — already unconditional on
/// the popup — reveals it); every tap after that, while it is still open,
/// mutes/unmutes instead, the same as clicking the button while hovering
/// on desktop. Closing it again is an outside tap (blurs it, same as
/// desktop losing hover) or the surrounding chrome fading on its own —
/// never a second tap on the trigger itself. Owns the slider's
/// write-through to `volume_by_peer` / `muted_by_peer` and the real
/// `<video>` element; the mute handler is passed in because it is shared
/// with the rest of the card.
#[component]
pub(super) fn VolumeControl<M>(
    /// Hidden unless this viewer is currently watching this stream.
    #[prop(into)]
    hidden: Signal<bool>,
    is_touch: ReadSignal<bool>,
    #[prop(into)] is_muted: Signal<bool>,
    /// Current level as a 0–100 percentage.
    #[prop(into)]
    volume_pct: Signal<f64>,
    /// The peer whose stream this controls, `None` for an empty slot.
    #[prop(into)]
    peer_id: Signal<Option<String>>,
    #[prop(into)] video_slot: Signal<VideoSlot>,
    volume_by_peer: RwSignal<std::collections::HashMap<String, f64>>,
    muted_by_peer: RwSignal<std::collections::HashSet<String>>,
    /// Toggles mute for this stream — shared with the card's other mute
    /// affordances, so it is owned by the parent. On touch it must not
    /// blur the trigger (see `apply_mute_toggle` in `watch_widgets.rs`),
    /// or muting would close the popup the same tap just opened it with.
    on_mute_toggle: M,
) -> impl IntoView
where
    M: Fn(leptos::ev::MouseEvent) + 'static,
{
    // Touch only: set from the trigger's `mousedown` (before the browser's
    // own focus-on-mousedown applies), read from its `click` right after,
    // to tell "this tap is opening the popup" from "the popup was already
    // open" — see `event_target_already_focused` and `QualityMenu`, which
    // uses the same trick for its own open/close.
    let trigger_was_open = RwSignal::new(false);

    view! {
        <div
            class="volume-control"
            class:hidden=move || hidden.get()
            on:click=move |ev: leptos::ev::MouseEvent| ev.stop_propagation()
        >
            <div class="volume-control__popup">
                <input
                    class="volume-control__slider"
                    type="range"
                    min="0"
                    max="100"
                    prop:value=move || if is_muted.get() { 0.0 } else { volume_pct.get() }
                    // Drives the filled portion of the track — CSS can't
                    // read a range's value on its own.
                    style=move || {
                        let pct = if is_muted.get() { 0.0 } else { volume_pct.get() };
                        format!("--volume-fill: {pct}%")
                    }
                    on:input:target=move |ev| {
                        let Some(pid) = peer_id.get() else { return };
                        let value = ev.target().value();
                        let volume = value.parse::<f64>().unwrap_or(100.0) / 100.0;
                        volume_by_peer.update(|m| {
                            m.insert(pid.clone(), volume);
                        });
                        set_volume(video_slot.get(), &pid, volume);
                        if volume > 0.0 && is_muted.get() {
                            muted_by_peer.update(|set| {
                                set.remove(&pid);
                            });
                            set_muted(video_slot.get(), &pid, false);
                        }
                    }
                    // Mouse: drop focus once the drag is committed so the
                    // popup (open on `:focus-within`) closes as soon as the
                    // pointer leaves. Touch keeps the popup up until an
                    // explicit close — an outside tap blurs the slider on
                    // its own — so fine-tuning the level doesn't dismiss it.
                    on:change=move |_| {
                        if !is_touch.get_untracked() {
                            blur_active_element();
                        }
                    }
                />
            </div>
            <button
                class="icon-btn icon-btn--neutral"
                title=move || if is_muted.get() { "Ativar som" } else { "Silenciar" }
                aria-label=move || if is_muted.get() { "Ativar som" } else { "Silenciar" }
                on:mousedown=move |ev: leptos::ev::MouseEvent| {
                    if is_touch.get_untracked() {
                        trigger_was_open.set(event_target_already_focused(&ev));
                    }
                }
                on:click=move |ev: leptos::ev::MouseEvent| {
                    // Touch: the first tap (button not focused yet) only
                    // opens the popup via the focus it just received —
                    // muting it too would be surprising before the slider
                    // was ever seen. Every tap after that mutes/unmutes,
                    // same as clicking this button while hovering it on
                    // desktop.
                    if is_touch.get_untracked() && !trigger_was_open.get() {
                        return;
                    }
                    on_mute_toggle(ev);
                }
            >
                {move || {
                    if is_muted.get() {
                        icon_volume_off().into_any()
                    } else {
                        icon_volume().into_any()
                    }
                }}
            </button>
        </div>
    }
}
