use leptos::prelude::*;

use super::connection::RoomConnection;
use super::media_controls::{toggle_fullscreen, toggle_picture_in_picture};
use super::watch::{stop_watching_click_handler, watch_click_handler};
use super::RoomMember;
use crate::signaling::protocol::MAX_MEMBERS;
use crate::ui::components::icons::{icon_eye, icon_maximize, icon_pip, icon_screen_off};
use crate::ui::components::palette::{avatar_letter, color_hex};

/// `MAX_MEMBERS` fixed, static cards, not a reactive `<For>` — the buttons
/// capture `RoomConnection` (`Rc<RefCell<...>>`, not Send + Sync, which
/// Leptos 0.8 requires of `<For>` children). Slot `i` shows whoever is in
/// position `i` of `members`, not a fixed member.
pub(super) fn member_cards(
    conn: RoomConnection,
    members: ReadSignal<Vec<RoomMember>>,
    my_peer_id: ReadSignal<Option<String>>,
    is_sharing: ReadSignal<bool>,
    watching: RwSignal<std::collections::HashSet<String>>,
    expanded: RwSignal<Option<String>>,
    watchers_by_sharer: RwSignal<std::collections::HashMap<String, Vec<String>>>,
    own_preview_hidden: RwSignal<bool>,
    hide_idle: RwSignal<bool>,
    connection_errors: RwSignal<std::collections::HashSet<String>>,
) -> Vec<impl IntoView> {
    (0..MAX_MEMBERS)
        .map(|i| {
            let member_at = move || members.get().get(i).cloned();
            let is_self = move || {
                member_at()
                    .zip(my_peer_id.get())
                    .map(|(m, my_id)| m.peer_id == my_id)
                    .unwrap_or(false)
            };
            let is_watching_this = move || {
                member_at().map(|m| watching.get().contains(&m.peer_id)).unwrap_or(false)
            };
            let can_watch = move || {
                member_at().map(|m| m.sharing).unwrap_or(false) && !is_self() && !is_watching_this()
            };
            // `RoomMember.sharing` is never `true` on one's own card — the
            // server only sends `PeerStartedSharing` to everyone else.
            let member_is_sharing = move || {
                member_at().map(|m| m.sharing).unwrap_or(false) || (is_self() && is_sharing.get())
            };
            let is_expanded = move || {
                member_at().map(|m| expanded.get().as_deref() == Some(m.peer_id.as_str())).unwrap_or(false)
            };
            let own_preview_visible = move || is_self() && is_sharing.get() && !own_preview_hidden.get();
            let showing_video = move || own_preview_visible() || (!is_self() && is_watching_this());
            let watcher_ids = move || {
                member_at().and_then(|m| watchers_by_sharer.get().get(&m.peer_id).cloned()).unwrap_or_default()
            };
            let watcher_names = move || {
                let ids = watcher_ids();
                let all_members = members.get();
                ids.iter()
                    .map(|id| {
                        all_members
                            .iter()
                            .find(|m| &m.peer_id == id)
                            .map(|m| m.nick.clone())
                            .unwrap_or_else(|| "alguém".to_string())
                    })
                    .collect::<Vec<_>>()
            };

            let watch = watch_click_handler(conn.clone(), members, watching, i);
            let stop_watch = stop_watching_click_handler(conn.clone(), members, watching, expanded, i);
            let fullscreen_click = move |ev: leptos::ev::MouseEvent| {
                ev.stop_propagation();
                let peer_id = member_at().map(|m| m.peer_id).unwrap_or_default();
                toggle_fullscreen(is_self(), &peer_id);
            };
            let pip_click = move |ev: leptos::ev::MouseEvent| {
                ev.stop_propagation();
                let peer_id = member_at().map(|m| m.peer_id).unwrap_or_default();
                toggle_picture_in_picture(is_self(), &peer_id);
            };
            let toggle_focus_click = move |_: leptos::ev::MouseEvent| {
                if !showing_video() {
                    return;
                }
                let Some(member) = member_at() else { return };
                if is_expanded() {
                    expanded.set(None);
                } else {
                    expanded.set(Some(member.peer_id));
                }
            };

            view! {
                <div
                    class="card"
                    class:hidden=move || {
                        let filtered_out_of_main_grid = expanded.get().is_none()
                            && ((hide_idle.get() && !member_is_sharing())
                                || (is_self() && own_preview_hidden.get()));
                        member_at().is_none() || filtered_out_of_main_grid
                    }
                    class:card--focus=is_expanded
                    class:card--clickable=showing_video
                    style=move || {
                        let (border, _bg) = member_at().map(|m| color_hex(&m.color)).unwrap_or(("#b0b8c1", "#2a2d31"));
                        format!("border-color: {border}; --member-accent: {border};")
                    }
                    on:click=toggle_focus_click
                >
                    <div
                        class="watcher-badge"
                        class:hidden=move || !member_is_sharing()
                    >
                        {icon_eye}
                        <span>{move || watcher_ids().len()}</span>
                        <div class="watcher-badge__tooltip" class:hidden=move || watcher_names().is_empty()>
                            {move || watcher_names().join(", ")}
                        </div>
                    </div>
                    <div
                        class="card__avatar"
                        class:hidden=showing_video
                        style=move || {
                            let (border, _bg) = member_at().map(|m| color_hex(&m.color)).unwrap_or(("#b0b8c1", "#2a2d31"));
                            format!("background-color: color-mix(in srgb, {border} 22%, var(--surface-2)); border-color: {border};")
                        }
                    >
                        <span class="card__avatar-letter">
                            {move || member_at().map(|m| avatar_letter(&m.nick)).unwrap_or_default()}
                        </span>
                    </div>
                    <video
                        id=move || member_at().map(|m| format!("video-self-{}", m.peer_id)).unwrap_or_default()
                        class:hidden=move || !(is_self() && showing_video())
                        autoplay=true
                        playsinline=true
                        muted=true
                    ></video>
                    <video
                        id=move || member_at().map(|m| format!("video-{}", m.peer_id)).unwrap_or_default()
                        class:hidden=move || !(!is_self() && showing_video())
                        autoplay=true
                        playsinline=true
                    ></video>
                    <div
                        class="card__error"
                        class:hidden=move || {
                            member_at().map(|m| !connection_errors.get().contains(&m.peer_id)).unwrap_or(true)
                        }
                    >
                        "Não foi possível conectar."
                    </div>
                    <button
                        class="card__watch-overlay"
                        class:hidden=move || !can_watch()
                        on:click=move |ev: leptos::ev::MouseEvent| {
                            ev.stop_propagation();
                            watch(ev);
                        }
                    >
                        <span class="card__watch-overlay-icon">"▶"</span>
                        <span>"Assistir compartilhamento"</span>
                    </button>
                    <div class="card__footer">
                        <span class="card__nick">{move || member_at().map(|m| m.nick).unwrap_or_default()}</span>
                        <div class="card__actions">
                            <button
                                class="icon-btn icon-btn--neutral"
                                class:hidden=move || !showing_video()
                                title="Tela cheia"
                                aria-label="Tela cheia"
                                on:click=fullscreen_click
                            >
                                {icon_maximize}
                            </button>
                            <button
                                class="icon-btn icon-btn--neutral"
                                class:hidden=move || !showing_video()
                                title="Picture-in-picture"
                                aria-label="Picture-in-picture"
                                on:click=pip_click
                            >
                                {icon_pip}
                            </button>
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
                        </div>
                    </div>
                </div>
            }
        })
        .collect::<Vec<_>>()
}
