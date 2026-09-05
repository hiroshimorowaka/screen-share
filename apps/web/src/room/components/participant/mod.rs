use leptos::prelude::*;

use crate::components::palette::{avatar_letter, color_hex};
use crate::room::media_controls::{
    exit_fullscreen_if_active, reveal_fullscreen_controls_if_active,
};
use crate::room::watch::watch_click_handler;
use crate::room::RoomSession;
use crate::room::RoomState;
use action_bar::CardActionBar;
use badges::CardBadges;
use screen_share_protocol::{QualityLevel, MAX_MEMBERS};

mod action_bar;
mod badges;
pub(super) mod parts;
mod watch_widgets;

/// Border/background used for a card slot that currently holds no member.
const EMPTY_SLOT_COLORS: (&str, &str) = ("#b0b8c1", "#2a2d31");

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
pub(super) fn quality_label(quality: QualityLevel) -> &'static str {
    match quality {
        QualityLevel::Auto => "Auto",
        QualityLevel::High => "Alta",
        QualityLevel::Medium => "Média",
        QualityLevel::Low => "Baixa",
    }
}

/// The four levels in the order the picker lists them — `Auto` first, then
/// worst-case fixed tiers descending.
pub(super) const QUALITY_LEVELS: [QualityLevel; 4] = [
    QualityLevel::Auto,
    QualityLevel::High,
    QualityLevel::Medium,
    QualityLevel::Low,
];

/// `MAX_MEMBERS` fixed card slots, not a reactive `<For>` — the buttons
/// capture `RoomSession` (`Rc<RefCell<...>>`, not Send + Sync, which
/// Leptos 0.8 requires of `<For>` children). Slot `i` shows whoever is in
/// position `i` of `members`, not a fixed member.
pub(crate) fn member_cards(conn: RoomSession) -> Vec<impl IntoView> {
    (0..MAX_MEMBERS)
        .map(|slot| {
            view! { <MemberCard conn=conn.clone() index=slot/> }
        })
        .collect::<Vec<_>>()
}

/// One card slot. Renders the member at position `slot` of the roster (or
/// nothing, hidden, when the slot is empty), the avatar / video / watch
/// affordances, and the card shell. The overlay badges and the action row
/// are their own components (`badges`, `action_bar`); the two stateful
/// widgets they use (quality, volume) live in `parts`.
#[component]
fn MemberCard(conn: RoomSession, index: usize) -> impl IntoView {
    let RoomState {
        members,
        my_peer_id,
        is_sharing,
        watching,
        expanded,
        own_preview_hidden,
        hide_idle,
        connection_errors,
        is_touch,
        controls_visible,
        ..
    } = expect_context::<RoomState>();
    let i = index;

    let member_at = move || members.get().get(i).cloned();
    let is_self = move || {
        member_at()
            .zip(my_peer_id.get())
            .is_some_and(|(m, my_id)| m.peer_id == my_id)
    };
    let is_watching_this = move || member_at().is_some_and(|m| watching.get().contains(&m.peer_id));
    let can_watch =
        move || member_at().is_some_and(|m| m.sharing) && !is_self() && !is_watching_this();
    // `RoomMember.sharing` is never `true` on one's own card — the server
    // only sends `PeerStartedSharing` to everyone else.
    let member_is_sharing =
        move || member_at().is_some_and(|m| m.sharing) || (is_self() && is_sharing.get());
    let is_expanded =
        move || member_at().is_some_and(|m| expanded.get().as_deref() == Some(m.peer_id.as_str()));
    let own_preview_visible = move || is_self() && is_sharing.get() && !own_preview_hidden.get();
    let showing_video = move || own_preview_visible() || (!is_self() && is_watching_this());
    let border_color = move || member_at().map_or(EMPTY_SLOT_COLORS, |m| color_hex(&m.color));

    let peer_video: NodeRef<leptos::html::Video> = NodeRef::new();
    rebind_slot_video(conn.clone(), members, peer_video, i);

    let watch = watch_click_handler(conn.clone(), members, watching, i);
    let card_click = move |ev: leptos::ev::MouseEvent| {
        // A tap/click on a fullscreen card: on desktop it backs out of
        // fullscreen (leaving the expanded/normal state untouched); on
        // touch it only reveals the idle-hidden controls — entering and
        // leaving fullscreen there is the "Tela cheia" button's job, so a
        // stray tap is no longer a one-way trip out.
        if is_touch.get_untracked() {
            if reveal_fullscreen_controls_if_active() {
                return;
            }
        } else if exit_fullscreen_if_active() {
            return;
        }
        // Discord-style: the whole tile is the "watch" affordance, not just
        // the small pill sitting on top of it.
        if can_watch() {
            watch(ev);
            // On a phone you watch one screen at a time — patching in goes
            // straight to focus, no roster tap-through.
            if is_touch.get_untracked() {
                if let Some(member) = member_at() {
                    expanded.set(Some(member.peer_id));
                }
            }
            return;
        }
        if !showing_video() {
            return;
        }
        let Some(member) = member_at() else { return };
        if is_touch.get_untracked() && is_expanded() {
            // In focus mode a tap on the video shows / hides the chrome
            // (Meet-style); leaving focus is the bar's back button, not a
            // tap.
            controls_visible.update(|v| *v = !*v);
            return;
        }
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
            class:card--self=is_self
            class:card--live=member_is_sharing
            class:card--patched=is_watching_this
            style=move || format!("--member: {};", border_color().0)
            on:click=card_click
        >
            <CardBadges index=i/>
            <div class="card__avatar" class:hidden=showing_video>
                <span class="card__avatar-letter">
                    {move || member_at().map_or_else(String::new, |m| avatar_letter(&m.nick))}
                </span>
            </div>
            <video
                id=move || {
                    member_at().map_or_else(String::new, |m| format!("video-self-{}", m.peer_id))
                }
                class:hidden=move || !is_self() || !showing_video()
                autoplay=true
                playsinline=true
                muted=true
            ></video>
            <video
                node_ref=peer_video
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
                <span class="card__nick">
                    {move || member_at().map(|m| m.nick).unwrap_or_default()}
                </span>
                <CardActionBar conn=conn.clone() index=i/>
            </div>
        </div>
    }
}

/// Keeps card slot `i`'s peer `<video>` pointed at the right inbound
/// stream. The grid has `MAX_MEMBERS` fixed slots and slot `i` renders
/// whoever is at roster position `i`, so a member leaving shifts everyone
/// after them down one slot — the `<video>` node stays put but now renders
/// a different member, stranding the `srcObject` that `ontrack` attached by
/// element id. Binding through the slot's `NodeRef` (stable across the
/// shift) instead of an id lookup, this effect re-points the node at the
/// current occupant's stream from `RoomSession::incoming_streams` on every
/// roster change, and clears it for a slot that no longer shows a stream we
/// hold.
#[cfg(feature = "hydrate")]
fn rebind_slot_video(
    conn: RoomSession,
    members: ReadSignal<Vec<crate::room::RoomMember>>,
    peer_video: NodeRef<leptos::html::Video>,
    i: usize,
) {
    Effect::new(move |_| {
        let Some(video) = peer_video.get() else {
            return;
        };
        let stream = members
            .get()
            .get(i)
            .and_then(|member| conn.incoming_streams.borrow().get(&member.peer_id).cloned());
        match stream {
            Some(stream) if video.src_object().as_ref() != Some(&stream) => {
                video.set_src_object(Some(&stream));
                if let Ok(promise) = video.play() {
                    leptos::task::spawn_local(async move {
                        let _ = wasm_bindgen_futures::JsFuture::from(promise).await;
                    });
                }
            }
            Some(_) => {}
            None if video.src_object().is_some() => video.set_src_object(None),
            None => {}
        }
    });
}

#[cfg(not(feature = "hydrate"))]
fn rebind_slot_video(
    _conn: RoomSession,
    _members: ReadSignal<Vec<crate::room::RoomMember>>,
    _peer_video: NodeRef<leptos::html::Video>,
    _i: usize,
) {
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
