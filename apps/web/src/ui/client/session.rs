use std::cell::RefCell;

use crate::ui::client::socket::WsClient;
use screen_share_protocol::{MemberInfo, TurnCredentials};

/// An already-authenticated connection that `HomePage` leaves ready for
/// `RoomPage` to take over, without reopening the WebSocket. `thread_local`
/// instead of Leptos context: `WsClient` only exists under `hydrate`, but
/// `App` also compiles under `ssr`.
pub struct PendingSession {
    pub room: String,
    pub room_name: String,
    pub ws: WsClient,
    pub peer_id: String,
    pub members: Vec<MemberInfo>,
    pub active_sharers: Vec<String>,
    pub requires_password: bool,
    pub turn: Option<TurnCredentials>,
}

thread_local! {
    static PENDING: RefCell<Option<PendingSession>> = const { RefCell::new(None) };
}

pub fn stash(session: PendingSession) {
    PENDING.with(|cell| *cell.borrow_mut() = Some(session));
}

/// Only takes it if it's for the requested room — avoids leaking into a
/// `RoomPage` different from the one the session was created for.
pub fn take(room: &str) -> Option<PendingSession> {
    PENDING.with(|cell| {
        let mut slot = cell.borrow_mut();
        if slot.as_ref().map(|s| s.room.as_str()) == Some(room) {
            slot.take()
        } else {
            None
        }
    })
}
