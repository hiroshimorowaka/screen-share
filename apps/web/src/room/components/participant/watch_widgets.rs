//! The "while watching this peer" cluster of a card's action row: the
//! quality menu, the volume control, and the stop-watching button. Split
//! out of `action_bar` so neither view grows past the line budget.

use std::collections::HashSet;

use leptos::prelude::*;
use screen_share_protocol::QualityLevel;

use super::parts::{QualityMenu, VolumeControl};
use crate::components::ui::icons::icon_screen_off;
use crate::room::media_controls::{blur_active_element, set_muted, VideoSlot};
use crate::room::watch::stop_watching_click_handler;
use crate::room::{RoomSession, RoomState};

/// Flip one peer's local mute: update the shared set, apply it to the
/// actual `<video>`, and drop focus off the button so a keyboard `Enter`
/// doesn't re-fire it.
fn apply_mute_toggle(
    peer_id: &str,
    slot: VideoSlot,
    muted_by_peer: RwSignal<HashSet<String>>,
    now_muted: bool,
) {
    muted_by_peer.update(|set| {
        if now_muted {
            set.insert(peer_id.to_string());
        } else {
            set.remove(peer_id);
        }
    });
    set_muted(slot, peer_id, now_muted);
    blur_active_element();
}

#[component]
pub(super) fn WatchWidgets(conn: RoomSession, index: usize) -> impl IntoView {
    let RoomState {
        members,
        my_peer_id,
        watching,
        expanded,
        is_touch,
        volume_by_peer,
        muted_by_peer,
        quality_by_peer,
        ..
    } = expect_context::<RoomState>();

    let member_at = move || members.get().get(index).cloned();
    let is_self = move || {
        member_at()
            .zip(my_peer_id.get())
            .is_some_and(|(m, my_id)| m.peer_id == my_id)
    };
    let is_watching_this = move || member_at().is_some_and(|m| watching.get().contains(&m.peer_id));
    let video_slot = move || {
        if is_self() {
            VideoSlot::Own
        } else {
            VideoSlot::Peer
        }
    };
    let is_muted = move || member_at().is_some_and(|m| muted_by_peer.get().contains(&m.peer_id));

    let stop_watch = stop_watching_click_handler(conn.clone(), members, watching, expanded, index);
    let set_quality =
        crate::room::quality::set_quality_handler(conn, members, quality_by_peer, index);
    let mute_toggle_click = move |ev: leptos::ev::MouseEvent| {
        ev.stop_propagation();
        if let Some(member) = member_at() {
            apply_mute_toggle(&member.peer_id, video_slot(), muted_by_peer, !is_muted());
        }
    };

    view! {
        <QualityMenu
            hidden=Signal::derive(move || !is_watching_this())
            is_touch=is_touch
            current=Signal::derive(move || {
                member_at()
                    .and_then(|m| quality_by_peer.get().get(&m.peer_id).copied())
                    .unwrap_or(QualityLevel::Auto)
            })
            on_select=set_quality
        />
        <VolumeControl
            hidden=Signal::derive(move || !is_watching_this())
            is_muted=Signal::derive(is_muted)
            volume_pct=Signal::derive(move || {
                let volume = member_at()
                    .and_then(|m| volume_by_peer.get().get(&m.peer_id).copied())
                    .unwrap_or(1.0);
                (volume * 100.0).round()
            })
            peer_id=Signal::derive(move || member_at().map(|m| m.peer_id))
            video_slot=Signal::derive(video_slot)
            volume_by_peer=volume_by_peer
            muted_by_peer=muted_by_peer
            on_mute_toggle=mute_toggle_click
        />
        <button
            class="icon-btn icon-btn--danger"
            class:hidden=move || !is_watching_this()
            title="Parar de assistir"
            aria-label="Parar de assistir"
            on:click=move |ev: leptos::ev::MouseEvent| {
                ev.stop_propagation();
                stop_watch(ev);
            }
        >
            {icon_screen_off}
        </button>
    }
}
