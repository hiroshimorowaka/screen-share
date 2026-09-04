//! The overlay badges on a card: the watcher count / names badge shown
//! while the member is sharing, and the corner cluster (ping-to-server
//! badge + "você" tag). Recomputes what it needs from `RoomState` and the
//! slot index so `MemberCard` doesn't have to thread it in.

use leptos::prelude::*;

use super::ping_color_var;
use crate::components::ui::icons::icon_eye;
use crate::room::RoomState;

#[component]
pub(super) fn CardBadges(index: usize) -> impl IntoView {
    let RoomState {
        members,
        my_peer_id,
        is_sharing,
        watchers_by_sharer,
        latency_by_peer,
        ..
    } = expect_context::<RoomState>();
    let i = index;

    let member_at = move || members.get().get(i).cloned();
    let is_self = move || {
        member_at()
            .zip(my_peer_id.get())
            .is_some_and(|(m, my_id)| m.peer_id == my_id)
    };
    // `RoomMember.sharing` is never `true` on one's own card — the server
    // only sends `PeerStartedSharing` to everyone else.
    let member_is_sharing =
        move || member_at().is_some_and(|m| m.sharing) || (is_self() && is_sharing.get());
    let member_ping =
        move || member_at().and_then(|m| latency_by_peer.get().get(&m.peer_id).copied());
    let watcher_ids = move || {
        member_at()
            .and_then(|m| watchers_by_sharer.get().get(&m.peer_id).cloned())
            .unwrap_or_default()
    };
    let watcher_names = move || {
        let ids = watcher_ids();
        let all_members = members.get();
        ids.iter()
            .map(|id| {
                all_members
                    .iter()
                    .find(|m| &m.peer_id == id)
                    .map_or_else(|| "alguém".to_string(), |m| m.nick.clone())
            })
            .collect::<Vec<_>>()
    };

    view! {
        <div class="watcher-badge" class:hidden=move || !member_is_sharing()>
            {icon_eye}
            <span>{move || watcher_ids().len()}</span>
            <div
                class="watcher-badge__tooltip"
                class:hidden=move || watcher_names().is_empty()
            >
                {move || {
                    watcher_names()
                        .into_iter()
                        .map(|name| view! { <span class="watcher-badge__name">{name}</span> })
                        .collect::<Vec<_>>()
                }}
            </div>
        </div>
        <div class="card__corner-start">
            <div
                class="ping-badge"
                class:hidden=move || member_ping().is_none()
                title="Ping até o servidor"
            >
                <span
                    class="ping-badge__dot"
                    style=move || {
                        format!(
                            "background-color: var({});",
                            member_ping().map_or("--text-dim", ping_color_var),
                        )
                    }
                ></span>
                <span>
                    {move || member_ping().map_or_else(String::new, |ms| format!("{ms} ms"))}
                </span>
            </div>
            <span class="card__self-tag" class:hidden=move || !is_self()>
                "você"
            </span>
        </div>
    }
}
