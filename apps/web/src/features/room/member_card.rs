use leptos::prelude::*;

use super::media_controls::{
    blur_active_element, exit_fullscreen_if_active, set_muted, set_volume, toggle_fullscreen,
    toggle_picture_in_picture, VideoSlot,
};
use super::watch::{stop_watching_click_handler, watch_click_handler};
use crate::components::icons::{
    icon_bars, icon_chevron_down, icon_eye, icon_maximize, icon_pip, icon_screen_off, icon_volume,
    icon_volume_off,
};
use crate::components::palette::{avatar_letter, color_hex};
use crate::session::RoomMember;
use crate::session::RoomSession;
use screen_share_protocol::{QualityLevel, MAX_MEMBERS};

/// Border/background used for a card slot that currently holds no member.
const EMPTY_SLOT_COLORS: (&str, &str) = ("#b0b8c1", "#2a2d31");

/// Everything a member card needs to render itself and react to room state.
#[derive(Clone, Copy)]
pub(super) struct MemberCardSignals {
    pub(super) members: ReadSignal<Vec<RoomMember>>,
    pub(super) my_peer_id: ReadSignal<Option<String>>,
    pub(super) is_sharing: ReadSignal<bool>,
    pub(super) watching: RwSignal<std::collections::HashSet<String>>,
    pub(super) expanded: RwSignal<Option<String>>,
    pub(super) watchers_by_sharer: RwSignal<std::collections::HashMap<String, Vec<String>>>,
    pub(super) own_preview_hidden: RwSignal<bool>,
    pub(super) hide_idle: RwSignal<bool>,
    pub(super) connection_errors: RwSignal<std::collections::HashSet<String>>,
    pub(super) volume_by_peer: RwSignal<std::collections::HashMap<String, f64>>,
    pub(super) muted_by_peer: RwSignal<std::collections::HashSet<String>>,
    pub(super) latency_by_peer: RwSignal<std::collections::HashMap<String, u32>>,
    pub(super) quality_by_peer:
        RwSignal<std::collections::HashMap<String, screen_share_protocol::QualityLevel>>,
}

/// Below this, a ping reads as "good" (green); below `PING_WARN_MS`, "ok"
/// (yellow); at or above it, "bad" (red) — the same three-tier read as a
/// signal-strength icon, just as a color instead of bars.
const PING_GOOD_MS: u32 = 60;
const PING_WARN_MS: u32 = 150;

/// Pure so it's unit-testable without a browser — see `mod tests` below.
fn ping_color_var(ms: u32) -> &'static str {
    if ms < PING_GOOD_MS {
        "--success"
    } else if ms < PING_WARN_MS {
        "--warning"
    } else {
        "--error"
    }
}

/// The label shown for each quality level in the picker.
fn quality_label(quality: QualityLevel) -> &'static str {
    match quality {
        QualityLevel::Auto => "Auto",
        QualityLevel::High => "Alta",
        QualityLevel::Medium => "Média",
        QualityLevel::Low => "Baixa",
    }
}

/// The four levels in the order the picker lists them — `Auto` first, then
/// worst-case fixed tiers descending.
const QUALITY_LEVELS: [QualityLevel; 4] = [
    QualityLevel::Auto,
    QualityLevel::High,
    QualityLevel::Medium,
    QualityLevel::Low,
];

/// `MAX_MEMBERS` fixed, static cards, not a reactive `<For>` — the buttons
/// capture `RoomSession` (`Rc<RefCell<...>>`, not Send + Sync, which
/// Leptos 0.8 requires of `<For>` children). Slot `i` shows whoever is in
/// position `i` of `members`, not a fixed member.
pub(super) fn member_cards(conn: RoomSession, signals: MemberCardSignals) -> Vec<impl IntoView> {
    let MemberCardSignals {
        members,
        my_peer_id,
        is_sharing,
        watching,
        expanded,
        watchers_by_sharer,
        own_preview_hidden,
        hide_idle,
        connection_errors,
        volume_by_peer,
        muted_by_peer,
        latency_by_peer,
        quality_by_peer,
    } = signals;

    (0..MAX_MEMBERS)
        .map(|i| {
            let member_at = move || members.get().get(i).cloned();
            let is_self = move || {
                member_at().zip(my_peer_id.get()).is_some_and(|(m, my_id)| m.peer_id == my_id)
            };
            let is_watching_this = move || {
                member_at().is_some_and(|m| watching.get().contains(&m.peer_id))
            };
            let can_watch = move || {
                member_at().is_some_and(|m| m.sharing) && !is_self() && !is_watching_this()
            };
            // `RoomMember.sharing` is never `true` on one's own card — the
            // server only sends `PeerStartedSharing` to everyone else.
            let member_is_sharing = move || {
                member_at().is_some_and(|m| m.sharing) || (is_self() && is_sharing.get())
            };
            let is_expanded = move || {
                member_at().is_some_and(|m| expanded.get().as_deref() == Some(m.peer_id.as_str()))
            };
            let own_preview_visible = move || is_self() && is_sharing.get() && !own_preview_hidden.get();
            let showing_video = move || own_preview_visible() || (!is_self() && is_watching_this());
            let border_color = move || {
                member_at().map_or(EMPTY_SLOT_COLORS, |m| color_hex(&m.color))
            };
            let member_ping = move || member_at().and_then(|m| latency_by_peer.get().get(&m.peer_id).copied());
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
                            .map_or_else(|| "alguém".to_string(), |m| m.nick.clone())
                    })
                    .collect::<Vec<_>>()
            };

            let watch = watch_click_handler(conn.clone(), members, watching, i);
            let stop_watch = stop_watching_click_handler(conn.clone(), members, watching, expanded, i);
            let video_slot = move || if is_self() { VideoSlot::Own } else { VideoSlot::Peer };
            let fullscreen_click = move |ev: leptos::ev::MouseEvent| {
                ev.stop_propagation();
                let peer_id = member_at().map_or(String::new(), |m| m.peer_id);
                toggle_fullscreen(video_slot(), &peer_id);
            };
            let pip_click = move |ev: leptos::ev::MouseEvent| {
                ev.stop_propagation();
                let peer_id = member_at().map_or(String::new(), |m| m.peer_id);
                toggle_picture_in_picture(video_slot(), &peer_id);
            };
            let is_muted = move || {
                member_at().is_some_and(|m| muted_by_peer.get().contains(&m.peer_id))
            };
            let current_volume_pct = move || {
                let volume = member_at()
                    .and_then(|m| volume_by_peer.get().get(&m.peer_id).copied())
                    .unwrap_or(1.0);
                (volume * 100.0).round()
            };
            let current_quality = move || {
                member_at()
                    .and_then(|m| quality_by_peer.get().get(&m.peer_id).copied())
                    .unwrap_or(QualityLevel::Auto)
            };
            let set_quality = crate::session::quality::set_quality_handler(conn.clone(), members, quality_by_peer, i);
            let mute_toggle_click = move |ev: leptos::ev::MouseEvent| {
                ev.stop_propagation();
                let Some(member) = member_at() else { return };
                let now_muted = !is_muted();
                muted_by_peer.update(|set| {
                    if now_muted {
                        set.insert(member.peer_id.clone());
                    } else {
                        set.remove(&member.peer_id);
                    }
                });
                set_muted(video_slot(), &member.peer_id, now_muted);
                blur_active_element();
            };
            let card_click = move |ev: leptos::ev::MouseEvent| {
                // Clicking a fullscreen card should just back out of
                // fullscreen and leave the expanded/normal state untouched —
                // not fall through and toggle it too (see
                // `exit_fullscreen_if_active`'s doc comment).
                if exit_fullscreen_if_active() {
                    return;
                }
                // Discord-style: the whole tile is the "watch" affordance,
                // not just the small pill sitting on top of it.
                if can_watch() {
                    watch(ev);
                    return;
                }
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
                    id=move || member_at().map_or_else(String::new, |m| format!("card-{}", m.peer_id))
                    class="card"
                    class:hidden=move || {
                        let filtered_out_of_main_grid = expanded.get().is_none()
                            && ((hide_idle.get() && !member_is_sharing())
                                || (is_self() && own_preview_hidden.get()));
                        member_at().is_none() || filtered_out_of_main_grid
                    }
                    class:card--focus=is_expanded
                    class:card--clickable=move || showing_video() || can_watch()
                    style=move || {
                        let border = border_color().0;
                        format!("border-color: {border}; --member-accent: {border};")
                    }
                    on:click=card_click
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
                        class="ping-badge"
                        class:hidden=move || member_ping().is_none()
                        title="Ping até o servidor"
                    >
                        <span
                            class="ping-badge__dot"
                            style=move || format!("background-color: var({});", member_ping().map_or("--text-dim", ping_color_var))
                        ></span>
                        <span>{move || member_ping().map_or_else(String::new, |ms| format!("{ms} ms"))}</span>
                    </div>
                    <div
                        class="card__avatar"
                        class:hidden=showing_video
                        style=move || {
                            let border = border_color().0;
                            format!("background-color: color-mix(in srgb, {border} 22%, var(--surface-2)); border-color: {border};")
                        }
                    >
                        <span class="card__avatar-letter">
                            {move || member_at().map_or_else(String::new, |m| avatar_letter(&m.nick))}
                        </span>
                    </div>
                    <video
                        id=move || member_at().map_or_else(String::new, |m| format!("video-self-{}", m.peer_id))
                        class:hidden=move || !is_self() || !showing_video()
                        autoplay=true
                        playsinline=true
                        muted=true
                    ></video>
                    <video
                        id=move || member_at().map_or_else(String::new, |m| format!("video-{}", m.peer_id))
                        class:hidden=move || is_self() || !showing_video()
                        autoplay=true
                        playsinline=true
                    ></video>
                    <div
                        class="card__error"
                        class:hidden=move || {
                            member_at().is_none_or(|m| !connection_errors.get().contains(&m.peer_id))
                        }
                    >
                        "Não foi possível conectar."
                    </div>
                    <div class="card__watch-scrim" class:hidden=move || !can_watch()></div>
                    <div class="card__watch-pill" class:hidden=move || !can_watch()>
                        "Assistir transmissão"
                    </div>
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
                            // A custom menu rather than a native `<select>`:
                            // the browser's option list can't be themed to
                            // match the rest of the card, and it looked
                            // jarringly out of place. Same hover/focus-reveal
                            // pattern as `.volume-control` next to it.
                            <div
                                class="quality-menu"
                                class:hidden=move || !is_watching_this()
                                on:click=move |ev: leptos::ev::MouseEvent| ev.stop_propagation()
                            >
                                <button
                                    type="button"
                                    class="quality-menu__trigger"
                                    title="Qualidade do vídeo"
                                    aria-label="Qualidade do vídeo"
                                >
                                    {icon_bars()}
                                    <span class="quality-menu__current">
                                        {move || quality_label(current_quality())}
                                    </span>
                                    {icon_chevron_down()}
                                </button>
                                <div class="quality-menu__popup">
                                    {QUALITY_LEVELS
                                        .into_iter()
                                        .map(move |level| {
                                            let set_quality = set_quality.clone();
                                            view! {
                                                <button
                                                    type="button"
                                                    class="quality-menu__option"
                                                    class:quality-menu__option--active=move || current_quality() == level
                                                    on:click=move |ev: leptos::ev::MouseEvent| {
                                                        ev.stop_propagation();
                                                        set_quality(level);
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
                            <div
                                class="volume-control"
                                class:hidden=move || !is_watching_this()
                                on:click=move |ev: leptos::ev::MouseEvent| ev.stop_propagation()
                            >
                                <div class="volume-control__popup">
                                    <input
                                        class="volume-control__slider"
                                        type="range"
                                        min="0"
                                        max="100"
                                        prop:value=move || if is_muted() { 0.0 } else { current_volume_pct() }
                                        // Drives the filled portion of the track — CSS can't
                                        // read a range's value on its own.
                                        style=move || {
                                            let pct = if is_muted() { 0.0 } else { current_volume_pct() };
                                            format!("--volume-fill: {pct}%")
                                        }
                                        on:input:target=move |ev| {
                                            let Some(member) = member_at() else { return };
                                            let value = ev.target().value();
                                            let volume = value.parse::<f64>().unwrap_or(100.0) / 100.0;
                                            volume_by_peer.update(|m| {
                                                m.insert(member.peer_id.clone(), volume);
                                            });
                                            set_volume(video_slot(), &member.peer_id, volume);
                                            if volume > 0.0 && is_muted() {
                                                muted_by_peer.update(|set| {
                                                    set.remove(&member.peer_id);
                                                });
                                                set_muted(video_slot(), &member.peer_id, false);
                                            }
                                        }
                                        // Drop focus once the drag is committed so the
                                        // popup (open on `:focus-within`) closes as soon
                                        // as the pointer leaves — same as the quality menu.
                                        on:change=move |_| blur_active_element()
                                    />
                                </div>
                                <button
                                    class="icon-btn icon-btn--neutral"
                                    title=move || if is_muted() { "Ativar som" } else { "Silenciar" }
                                    aria-label=move || if is_muted() { "Ativar som" } else { "Silenciar" }
                                    on:click=mute_toggle_click
                                >
                                    {move || if is_muted() { icon_volume_off().into_any() } else { icon_volume().into_any() }}
                                </button>
                            </div>
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

#[cfg(test)]
#[path = "member_card_tests.rs"]
mod tests;
