//! Dev-only visual test bench for the room page. Populates the exact same
//! grid/card/control-bar components the real `RoomPage` uses with fake,
//! locally-managed members — no `WsClient`, no signaling, no WebRTC. Lets
//! you eyeball how the layout, focus mode, and watch/leave interactions
//! behave with any number/mix of members without a second browser tab or a
//! real signaling round-trip.
//!
//! This whole module only exists in debug builds — see the
//! `#[cfg(debug_assertions)]` on its `mod` declaration in `room/mod.rs`, and
//! on the route that serves it in `app.rs`. A release build never compiles
//! this file, so there's no dev-only route or code path to accidentally
//! ship.

use leptos::prelude::*;

use super::components::participant::member_cards;
use super::components::participant_grid::{setup_adaptive_grid, setup_auto_hide_controls};
use super::media_controls::setup_fullscreen_autohide_controls;
use super::watch::leave_or_stop_watching_handler;
use crate::components::palette::{color_hex, palette_ids, DEFAULT_COLOR};
use crate::components::ui::color_picker::ColorPicker;
use crate::components::ui::icons::{icon_eye_off, icon_log_out, icon_screen_off, icon_video_off};
use crate::room::RoomMember;
use crate::room::RoomSession;
use crate::room::RoomState;
use screen_share_protocol::MAX_MEMBERS;

/// Bulk-add nicknames cycle through this so "adicionar vários" gives you an
/// instantly readable room without typing.
const BULK_ADD_NICKS: &[&str] = &[
    "Ana", "Bia", "Caio", "Dudu", "Eva", "Fefe", "Gil", "Hugo", "Ivo", "Joca",
];

fn next_palette_color(current: &str) -> &'static str {
    let ids: Vec<&str> = palette_ids().collect();
    let current_index = ids.iter().position(|id| *id == current).unwrap_or(0);
    ids[(current_index + 1) % ids.len()]
}

// Debug-only (`#[cfg(debug_assertions)]`) visual harness that fabricates
// a full room to eyeball card states without a live session. Long by
// nature — it hand-builds fixture data — and never ships in a release
// build, so it is exempt rather than queued for a split.
#[allow(clippy::too_many_lines)]
#[component]
pub(crate) fn DevRoomPreviewPage() -> impl IntoView {
    // The dev bench drives the exact same `<MemberCard>` components the
    // real room does, so it builds a `RoomState` and `provide_context`s it
    // the same way `RoomPage` does — then destructures the handles its own
    // fixture controls poke.
    let state = RoomState::new();
    provide_context(state);
    let RoomState {
        members,
        set_members,
        my_peer_id,
        set_my_peer_id,
        is_sharing,
        set_is_sharing,
        watching,
        expanded,
        watchers_by_sharer,
        hide_idle,
        controls_visible,
        is_touch,
        own_preview_hidden,
        latency_by_peer,
        ..
    } = state;
    let (next_id, set_next_id) = signal(0u32);
    let panel_open = RwSignal::new(true);

    let (new_nick, set_new_nick) = signal(String::new());
    let (new_color, set_new_color) = signal(DEFAULT_COLOR.to_string());

    let conn = RoomSession::new();

    let take_next_id = move || {
        let id = next_id.get_untracked();
        set_next_id.set(id + 1);
        format!("fake-{id}")
    };

    let add_member = move |nick: String, color: String| {
        let nick = nick.trim().to_string();
        if nick.is_empty() || members.get_untracked().len() >= MAX_MEMBERS {
            return;
        }
        let peer_id = take_next_id();
        set_members.update(|members| {
            members.push(RoomMember {
                peer_id,
                nick,
                color,
                sharing: false,
            })
        });
    };

    let add_from_form = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        add_member(new_nick.get_untracked(), new_color.get_untracked());
        set_new_nick.set(String::new());
    };

    let add_bulk = move |_| {
        let room_size = members.get_untracked().len();
        let slots_left = MAX_MEMBERS.saturating_sub(room_size);
        for i in 0..slots_left.min(5) {
            let n = room_size + i;
            let nick = BULK_ADD_NICKS
                .get(n % BULK_ADD_NICKS.len())
                .copied()
                .unwrap_or("Membro");
            let color = palette_ids().nth(n % 10).unwrap_or(DEFAULT_COLOR);
            add_member(nick.to_string(), color.to_string());
        }
    };

    let remove_member = move |peer_id: String| {
        set_members.update(|members| members.retain(|m| m.peer_id != peer_id));
        watching.update(|w| {
            w.remove(&peer_id);
        });
        watchers_by_sharer.update(|w| {
            w.remove(&peer_id);
            for watchers in w.values_mut() {
                watchers.retain(|id| id != &peer_id);
            }
        });
        expanded.update(|current| {
            if current.as_deref() == Some(peer_id.as_str()) {
                *current = None;
            }
        });
        if my_peer_id.get_untracked().as_deref() == Some(peer_id.as_str()) {
            set_my_peer_id.set(None);
            set_is_sharing.set(false);
        }
    };

    let toggle_sharing = move |peer_id: String| {
        if my_peer_id.get_untracked().as_deref() == Some(peer_id.as_str()) {
            set_is_sharing.update(|v| *v = !*v);
            return;
        }
        set_members.update(|members| {
            if let Some(m) = members.iter_mut().find(|m| m.peer_id == peer_id) {
                m.sharing = !m.sharing;
            }
        });
    };

    let cycle_color = move |peer_id: String| {
        set_members.update(|members| {
            if let Some(m) = members.iter_mut().find(|m| m.peer_id == peer_id) {
                m.color = next_palette_color(&m.color).to_string();
            }
        });
    };

    /// Cycles through the three ping-badge color tiers plus "no reading
    /// yet" — lets you eyeball all four states without a real signaling
    /// round trip.
    const PING_CYCLE_MS: &[u32] = &[15, 90, 220];
    let cycle_latency = move |peer_id: String| {
        latency_by_peer.update(|latencies| {
            let next = match latencies.get(&peer_id) {
                None => Some(PING_CYCLE_MS[0]),
                Some(&ms) if ms == PING_CYCLE_MS[0] => Some(PING_CYCLE_MS[1]),
                Some(&ms) if ms == PING_CYCLE_MS[1] => Some(PING_CYCLE_MS[2]),
                Some(_) => None,
            };
            match next {
                Some(ms) => {
                    latencies.insert(peer_id, ms);
                }
                None => {
                    latencies.remove(&peer_id);
                }
            }
        });
    };

    // The server never reports a viewer count for one's own card (see
    // `member_card.rs`) — marking someone "eu" here mirrors that: their
    // `sharing` flag moves out of the members list and into the room-level
    // `is_sharing` signal, same as the real protocol.
    let mark_as_self = move |peer_id: String| {
        set_my_peer_id.set(Some(peer_id.clone()));
        set_is_sharing.set(false);
        set_members.update(|members| {
            if let Some(m) = members.iter_mut().find(|m| m.peer_id == peer_id) {
                m.sharing = false;
            }
        });
    };

    let toggle_self = move |peer_id: String| {
        if my_peer_id.get_untracked().as_deref() == Some(peer_id.as_str()) {
            set_my_peer_id.set(None);
            set_is_sharing.set(false);
        } else {
            mark_as_self(peer_id);
        }
    };

    let adjust_watchers = move |peer_id: String, delta: i32| {
        watchers_by_sharer.update(|map| {
            let list = map.entry(peer_id).or_default();
            if delta > 0 {
                list.push(format!("watcher-{}", list.len() + 1));
            } else {
                list.pop();
            }
        });
    };

    let member_is_sharing = move |peer_id: String| {
        if my_peer_id.get().as_deref() == Some(peer_id.as_str()) {
            is_sharing.get()
        } else {
            members
                .get()
                .iter()
                .any(|m| m.peer_id == peer_id && m.sharing)
        }
    };

    let leave_or_stop_watching = leave_or_stop_watching_handler(
        conn.clone(),
        watching,
        expanded,
        my_peer_id,
        "dev-preview".to_string(),
    );
    let (pause_hide_controls, resume_hide_controls) =
        setup_auto_hide_controls(controls_visible, is_touch, expanded);
    setup_adaptive_grid(members, hide_idle, own_preview_hidden, is_sharing, expanded);
    setup_fullscreen_autohide_controls();

    view! {
        <div class="room-page">
            <div class="stage-header">
                <span class="status-row__meta">"Bancada de testes (só em dev)"</span>
                <span class="room-member-count">{move || format!("{}/{}", members.get().len(), MAX_MEMBERS)}</span>
                <span class="status-row__spacer"></span>
                <button
                    type="button"
                    class="btn btn--ghost"
                    on:click=move |_| panel_open.update(|v| *v = !*v)
                >
                    {move || if panel_open.get() { "Esconder painel" } else { "Mostrar painel" }}
                </button>
            </div>
            <div id="member-grid" class="grid" class:grid--focused=move || expanded.get().is_some()>
                {member_cards(conn)}
            </div>
            <div
                class="room-controls"
                class:room-controls--hidden=move || !controls_visible.get()
                on:mouseenter=move |_| pause_hide_controls()
                on:mouseleave=move |_| resume_hide_controls()
            >
                <div class="control-group">
                    <button
                        type="button"
                        class="icon-btn icon-btn--neutral"
                        class:icon-btn--active=hide_idle
                        title="Ocultar quem não está transmitindo"
                        aria-label="Ocultar quem não está transmitindo"
                        on:click=move |_| hide_idle.update(|v| *v = !*v)
                    >
                        {icon_eye_off}
                    </button>
                    <button
                        type="button"
                        class="icon-btn icon-btn--neutral"
                        class:icon-btn--active=own_preview_hidden
                        class:hidden=move || !is_sharing.get()
                        title=move || if own_preview_hidden.get() { "Mostrar meu preview" } else { "Esconder meu preview" }
                        aria-label="Esconder meu preview"
                        on:click=move |_| {
                            let now_hidden = !own_preview_hidden.get_untracked();
                            own_preview_hidden.set(now_hidden);
                            if now_hidden && expanded.get_untracked() == my_peer_id.get_untracked() {
                                expanded.set(None);
                            }
                        }
                    >
                        {icon_video_off}
                    </button>
                </div>
                <div class="control-group control-group--danger">
                    <button
                        type="button"
                        class="icon-btn icon-btn--danger"
                        title=move || if expanded.get().is_some() { "Parar de assistir" } else { "Sair da bancada" }
                        aria-label=move || if expanded.get().is_some() { "Parar de assistir" } else { "Sair da bancada" }
                        on:click=leave_or_stop_watching
                    >
                        {move || if expanded.get().is_some() { icon_screen_off().into_any() } else { icon_log_out().into_any() }}
                    </button>
                </div>
            </div>

            <div class="dev-panel" class:hidden=move || !panel_open.get()>
                <form class="dev-panel__add" on:submit=add_from_form>
                    <label class="field">
                        <span class="field__label">"Nick"</span>
                        <input
                            class="field__input"
                            type="text"
                            placeholder="Nome do membro fake"
                            prop:value=new_nick
                            on:input:target=move |ev| set_new_nick.set(ev.target().value())
                        />
                    </label>
                    <ColorPicker selected=new_color on_select=set_new_color/>
                    <div class="dev-panel__add-actions">
                        <button type="submit" class="btn btn--primary">"Adicionar membro"</button>
                        <button type="button" class="btn btn--ghost" on:click=add_bulk>"+5 aleatórios"</button>
                    </div>
                </form>

                <div class="dev-panel__list">
                    <For each=move || members.get() key=|m| m.peer_id.clone() let(member)>
                        {
                            let (border, _) = color_hex(&member.color);
                            let peer_id_for_swatch = member.peer_id.clone();
                            let peer_id_for_latency = member.peer_id.clone();
                            let peer_id_for_latency_label = member.peer_id.clone();
                            let peer_id_for_self = member.peer_id.clone();
                            let peer_id_for_self_label = member.peer_id.clone();
                            let peer_id_for_share = member.peer_id.clone();
                            let peer_id_for_share_label = member.peer_id.clone();
                            let peer_id_for_share_text = member.peer_id.clone();
                            let peer_id_for_watchers_visibility = member.peer_id.clone();
                            let peer_id_for_watchers_count = member.peer_id.clone();
                            let peer_id_for_watchers_minus = member.peer_id.clone();
                            let peer_id_for_watchers_plus = member.peer_id.clone();
                            let peer_id_for_remove = member.peer_id.clone();

                            view! {
                                <div class="dev-panel__row">
                                    <button
                                        type="button"
                                        class="dev-panel__swatch"
                                        style=format!("background-color: {border}")
                                        title="Trocar cor"
                                        on:click=move |_| cycle_color(peer_id_for_swatch.clone())
                                    ></button>
                                    <span class="dev-panel__nick">{member.nick.clone()}</span>
                                    <button
                                        type="button"
                                        class="btn btn--ghost"
                                        title="Alternar ping simulado"
                                        on:click=move |_| cycle_latency(peer_id_for_latency.clone())
                                    >
                                        {move || {
                                            latency_by_peer.get().get(&peer_id_for_latency_label).map_or_else(
                                                || "Sem ping".to_string(),
                                                |ms| format!("{ms} ms"),
                                            )
                                        }}
                                    </button>
                                    <button
                                        type="button"
                                        class="btn btn--ghost"
                                        class:icon-btn--active=move || my_peer_id.get().as_deref() == Some(peer_id_for_self_label.as_str())
                                        on:click=move |_| toggle_self(peer_id_for_self.clone())
                                    >
                                        "Eu"
                                    </button>
                                    <button
                                        type="button"
                                        class="btn btn--ghost"
                                        class:icon-btn--active=move || member_is_sharing(peer_id_for_share_label.clone())
                                        on:click=move |_| toggle_sharing(peer_id_for_share.clone())
                                    >
                                        {move || if member_is_sharing(peer_id_for_share_text.clone()) { "Transmitindo" } else { "Parado" }}
                                    </button>
                                    <div
                                        class="dev-panel__watchers"
                                        class:hidden=move || !member_is_sharing(peer_id_for_watchers_visibility.clone())
                                    >
                                        <button type="button" class="btn btn--ghost" on:click=move |_| adjust_watchers(peer_id_for_watchers_minus.clone(), -1)>"−"</button>
                                        <span>{move || watchers_by_sharer.get().get(&peer_id_for_watchers_count).map(Vec::len).unwrap_or(0)} " assistindo"</span>
                                        <button type="button" class="btn btn--ghost" on:click=move |_| adjust_watchers(peer_id_for_watchers_plus.clone(), 1)>"+"</button>
                                    </div>
                                    <button type="button" class="btn btn--ghost" on:click=move |_| remove_member(peer_id_for_remove.clone())>"Remover"</button>
                                </div>
                            }
                        }
                    </For>
                </div>
            </div>
        </div>
    }
}
