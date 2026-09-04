//! A card's action row: fullscreen / picture-in-picture (while a video is
//! showing) and, while watching, the quality / volume / stop-watching
//! cluster (`watch_widgets`). Recomputes what it needs from `RoomState`
//! and the slot index.

use leptos::prelude::*;

use super::watch_widgets::WatchWidgets;
use crate::components::ui::icons::{icon_maximize, icon_pip};
use crate::room::media_controls::{toggle_fullscreen, toggle_picture_in_picture, VideoSlot};
use crate::room::{RoomSession, RoomState};

#[component]
pub(super) fn CardActionBar(conn: RoomSession, index: usize) -> impl IntoView {
    let RoomState {
        members,
        my_peer_id,
        is_sharing,
        watching,
        own_preview_hidden,
        ..
    } = expect_context::<RoomState>();

    let member_at = move || members.get().get(index).cloned();
    let is_self = move || {
        member_at()
            .zip(my_peer_id.get())
            .is_some_and(|(m, my_id)| m.peer_id == my_id)
    };
    let is_watching_this = move || member_at().is_some_and(|m| watching.get().contains(&m.peer_id));
    let showing_video = move || {
        (is_self() && is_sharing.get() && !own_preview_hidden.get())
            || (!is_self() && is_watching_this())
    };
    let video_slot = move || {
        if is_self() {
            VideoSlot::Own
        } else {
            VideoSlot::Peer
        }
    };

    // `toggle_fullscreen` / `toggle_picture_in_picture` share the same
    // shape — stop the click bubbling to the card, then act on this card's
    // video slot — so they come from one combinator.
    let video_action = move |act: fn(VideoSlot, &str)| {
        move |ev: leptos::ev::MouseEvent| {
            ev.stop_propagation();
            act(
                video_slot(),
                &member_at().map_or(String::new(), |m| m.peer_id),
            );
        }
    };

    view! {
        <div class="card__actions">
            <button
                class="icon-btn icon-btn--neutral"
                class:hidden=move || !showing_video()
                title="Tela cheia"
                aria-label="Tela cheia"
                on:click=video_action(toggle_fullscreen)
            >
                {icon_maximize}
            </button>
            <button
                class="icon-btn icon-btn--neutral"
                class:hidden=move || !showing_video()
                title="Picture-in-picture"
                aria-label="Picture-in-picture"
                on:click=video_action(toggle_picture_in_picture)
            >
                {icon_pip}
            </button>
            <WatchWidgets conn=conn index=index/>
        </div>
    }
}
