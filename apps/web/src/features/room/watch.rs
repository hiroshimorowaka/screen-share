use leptos::prelude::*;

use crate::session::RoomMember;
use crate::session::RoomSession;

#[cfg(not(feature = "hydrate"))]
pub(super) fn watch_click_handler(
    _conn: RoomSession,
    _members: ReadSignal<Vec<RoomMember>>,
    _watching: RwSignal<std::collections::HashSet<String>>,
    _slot: usize,
) -> impl Fn(leptos::ev::MouseEvent) + Clone + 'static {
    move |_| {}
}

#[cfg(feature = "hydrate")]
pub(super) fn watch_click_handler(
    conn: RoomSession,
    members: ReadSignal<Vec<RoomMember>>,
    watching: RwSignal<std::collections::HashSet<String>>,
    slot: usize,
) -> impl Fn(leptos::ev::MouseEvent) + Clone + 'static {
    use screen_share_protocol::ClientMessage;

    move |_| {
        let Some(member) = members.get_untracked().get(slot).cloned() else {
            return;
        };
        watching.update(|w| {
            w.insert(member.peer_id.clone());
        });
        if let Some(ws) = conn.ws.borrow().as_ref() {
            ws.send(&ClientMessage::WatchShare {
                sharer_id: member.peer_id,
            });
        }
    }
}

#[cfg(not(feature = "hydrate"))]
pub(super) fn stop_watching_click_handler(
    _conn: RoomSession,
    _members: ReadSignal<Vec<RoomMember>>,
    _watching: RwSignal<std::collections::HashSet<String>>,
    _expanded: RwSignal<Option<String>>,
    _slot: usize,
) -> impl Fn(leptos::ev::MouseEvent) + Clone + 'static {
    move |_| {}
}

#[cfg(feature = "hydrate")]
pub(super) fn stop_watching_click_handler(
    conn: RoomSession,
    members: ReadSignal<Vec<RoomMember>>,
    watching: RwSignal<std::collections::HashSet<String>>,
    expanded: RwSignal<Option<String>>,
    slot: usize,
) -> impl Fn(leptos::ev::MouseEvent) + Clone + 'static {
    use screen_share_protocol::ClientMessage;

    move |_| {
        let Some(member) = members.get_untracked().get(slot).cloned() else {
            return;
        };
        watching.update(|w| {
            w.remove(&member.peer_id);
        });
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
        conn.incoming_callbacks.borrow_mut().remove(&member.peer_id);
        if let Some(ws) = conn.ws.borrow().as_ref() {
            ws.send(&ClientMessage::StopWatching {
                sharer_id: member.peer_id,
            });
        }
    }
}

/// Disconnects and sends this member home — a real, deliberate leave,
/// unlike a dropped connection (reload, closed tab), which is why it also
/// clears the saved room session so the nick/password gate shows again on
/// the next visit. Shared by the "leave" button
/// (`leave_or_stop_watching_handler`) and the quick-share flow's
/// cancelled-picker path (`share::start_sharing`'s `on_cancelled`).
#[cfg(not(feature = "hydrate"))]
#[allow(dead_code)]
pub(super) fn leave_room(
    _conn: &RoomSession,
    _room_code: &str,
    _my_peer_id: ReadSignal<Option<String>>,
) {
}

#[cfg(feature = "hydrate")]
pub(super) fn leave_room(
    conn: &RoomSession,
    room_code: &str,
    my_peer_id: ReadSignal<Option<String>>,
) {
    use leptos_router::hooks::use_navigate;

    // Leaving while still sharing otherwise strands Chrome's native
    // "you're sharing" bar / tab indicator — the capture tracks are never
    // stopped and the preview `<video>` keeps its `srcObject`. Run the same
    // teardown a deliberate "stop sharing" does before disconnecting. Bind
    // the `borrow()` to a local first so it is released before
    // `teardown_local_share` takes its own `borrow_mut()`.
    let was_sharing = conn.local_stream.borrow().is_some();
    if was_sharing {
        let me = my_peer_id.get_untracked();
        crate::session::media::teardown_local_share(conn, me.as_deref());
    }

    crate::infra::storage::clear_room_session(room_code);
    // Same teardown every non-button exit gets (see `session::reconnect`):
    // mark the close expected so the reconnect loop doesn't treat it as a
    // drop, stop any in-flight reconnect, drop the peer connections, and
    // take the socket out of its `RefCell` so the session actually frees.
    crate::session::reconnect::teardown_session(conn);
    let navigate = use_navigate();
    navigate("/", Default::default());
}

#[cfg(not(feature = "hydrate"))]
pub(super) fn leave_or_stop_watching_handler(
    _conn: RoomSession,
    _watching: RwSignal<std::collections::HashSet<String>>,
    _expanded: RwSignal<Option<String>>,
    _my_peer_id: ReadSignal<Option<String>>,
    _room_code: String,
) -> impl Fn(leptos::ev::MouseEvent) + Clone + 'static {
    move |_| {}
}

#[cfg(feature = "hydrate")]
pub(super) fn leave_or_stop_watching_handler(
    conn: RoomSession,
    watching: RwSignal<std::collections::HashSet<String>>,
    expanded: RwSignal<Option<String>>,
    my_peer_id: ReadSignal<Option<String>>,
    room_code: String,
) -> impl Fn(leptos::ev::MouseEvent) + Clone + 'static {
    use screen_share_protocol::ClientMessage;

    move |_| {
        let Some(focused_peer_id) = expanded.get_untracked() else {
            leave_room(&conn, &room_code, my_peer_id);
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
        conn.incoming_callbacks
            .borrow_mut()
            .remove(&focused_peer_id);
        if let Some(ws) = conn.ws.borrow().as_ref() {
            ws.send(&ClientMessage::StopWatching {
                sharer_id: focused_peer_id,
            });
        }
    }
}
