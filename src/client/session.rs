use std::cell::RefCell;

use crate::client::socket::WsClient;
use crate::signaling::protocol::MemberInfo;

/// Conexão já autenticada que `HomePage` deixa pronta pra `RoomPage`
/// assumir, sem reabrir o WebSocket. `thread_local` em vez de contexto do
/// Leptos: `WsClient` só existe sob `hydrate`, mas `App` também compila
/// sob `ssr`.
pub struct PendingSession {
    pub room: String,
    pub room_name: String,
    pub ws: WsClient,
    pub peer_id: String,
    pub members: Vec<MemberInfo>,
    pub active_sharers: Vec<String>,
}

thread_local! {
    static PENDING: RefCell<Option<PendingSession>> = const { RefCell::new(None) };
}

pub fn stash(session: PendingSession) {
    PENDING.with(|cell| *cell.borrow_mut() = Some(session));
}

/// Só retira se for pra sala pedida — evita vazar pra uma `RoomPage`
/// diferente da que a sessão foi criada para.
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
