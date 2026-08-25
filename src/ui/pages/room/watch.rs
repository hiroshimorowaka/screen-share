use leptos::prelude::*;

use super::connection::RoomConnection;
use super::RoomMember;

#[cfg(not(feature = "hydrate"))]
pub(super) fn watch_click_handler(
    _conn: RoomConnection,
    _members: ReadSignal<Vec<RoomMember>>,
    _watching: RwSignal<std::collections::HashSet<String>>,
    _slot: usize,
) -> impl Fn(leptos::ev::MouseEvent) + Clone + 'static {
    move |_| {}
}

#[cfg(feature = "hydrate")]
pub(super) fn watch_click_handler(
    conn: RoomConnection,
    members: ReadSignal<Vec<RoomMember>>,
    watching: RwSignal<std::collections::HashSet<String>>,
    slot: usize,
) -> impl Fn(leptos::ev::MouseEvent) + Clone + 'static {
    use crate::signaling::protocol::ClientMessage;

    move |_| {
        let Some(member) = members.get_untracked().get(slot).cloned() else { return };
        watching.update(|w| { w.insert(member.peer_id.clone()); });
        if let Some(ws) = conn.ws.borrow().as_ref() {
            ws.send(&ClientMessage::WatchShare { sharer_id: member.peer_id });
        }
    }
}

#[cfg(not(feature = "hydrate"))]
pub(super) fn stop_watching_click_handler(
    _conn: RoomConnection,
    _members: ReadSignal<Vec<RoomMember>>,
    _watching: RwSignal<std::collections::HashSet<String>>,
    _expanded: RwSignal<Option<String>>,
    _slot: usize,
) -> impl Fn(leptos::ev::MouseEvent) + Clone + 'static {
    move |_| {}
}

#[cfg(feature = "hydrate")]
pub(super) fn stop_watching_click_handler(
    conn: RoomConnection,
    members: ReadSignal<Vec<RoomMember>>,
    watching: RwSignal<std::collections::HashSet<String>>,
    expanded: RwSignal<Option<String>>,
    slot: usize,
) -> impl Fn(leptos::ev::MouseEvent) + Clone + 'static {
    use crate::signaling::protocol::ClientMessage;

    move |_| {
        let Some(member) = members.get_untracked().get(slot).cloned() else { return };
        watching.update(|w| { w.remove(&member.peer_id); });
        // The fullscreen API has no idea the video feeding it is about to
        // disappear — back out of it ourselves, the same way a peer
        // stopping their share or leaving the room already does (see
        // `message_handler.rs`), instead of leaving the browser stuck
        // showing a fullscreen card with nothing to watch anymore.
        let was_fullscreen = super::media_controls::exit_fullscreen_if_showing(&member.peer_id);
        expanded.update(|current| {
            if current.as_deref() == Some(member.peer_id.as_str()) || was_fullscreen {
                *current = None;
            }
        });
        if let Some(pc) = conn.incoming.borrow_mut().remove(&member.peer_id) {
            pc.close();
        }
        if let Some(ws) = conn.ws.borrow().as_ref() {
            ws.send(&ClientMessage::StopWatching { sharer_id: member.peer_id });
        }
    }
}

#[cfg(not(feature = "hydrate"))]
pub(super) fn leave_or_stop_watching_handler(
    _conn: RoomConnection,
    _watching: RwSignal<std::collections::HashSet<String>>,
    _expanded: RwSignal<Option<String>>,
    _my_peer_id: ReadSignal<Option<String>>,
    _room_code: String,
) -> impl Fn(leptos::ev::MouseEvent) + Clone + 'static {
    move |_| {}
}

#[cfg(feature = "hydrate")]
pub(super) fn leave_or_stop_watching_handler(
    conn: RoomConnection,
    watching: RwSignal<std::collections::HashSet<String>>,
    expanded: RwSignal<Option<String>>,
    my_peer_id: ReadSignal<Option<String>>,
    room_code: String,
) -> impl Fn(leptos::ev::MouseEvent) + Clone + 'static {
    use leptos_router::hooks::use_navigate;

    use crate::signaling::protocol::ClientMessage;

    move |_| {
        let Some(focused_peer_id) = expanded.get_untracked() else {
            // An active, deliberate leave — unlike a dropped connection
            // (reload, closed tab), this one should require the nick/
            // password gate again on the next visit.
            crate::ui::client::storage::clear_room_session(&room_code);
            if let Some(ws) = conn.ws.borrow().as_ref() {
                ws.close();
            }
            let navigate = use_navigate();
            navigate("/", Default::default());
            return;
        };

        super::media_controls::exit_fullscreen_if_showing(&focused_peer_id);
        expanded.set(None);
        if my_peer_id.get_untracked().as_deref() == Some(focused_peer_id.as_str()) {
            return;
        }

        watching.update(|w| {
            w.remove(&focused_peer_id);
        });
        if let Some(pc) = conn.incoming.borrow_mut().remove(&focused_peer_id) {
            pc.close();
        }
        if let Some(ws) = conn.ws.borrow().as_ref() {
            ws.send(&ClientMessage::StopWatching { sharer_id: focused_peer_id });
        }
    }
}
