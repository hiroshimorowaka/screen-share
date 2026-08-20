use std::cell::RefCell;

use crate::client::socket::WsClient;
use crate::signaling::protocol::MemberInfo;

/// Uma conexão já autenticada (via `CreateRoom`) que a `HomePage` deixa
/// pronta pra `RoomPage` assumir, sem reabrir o WebSocket nem repetir a
/// senha. Guardado num `thread_local` — não passa pelo sistema de contexto
/// do Leptos porque `WsClient` só existe sob a feature `hydrate`, e o
/// componente `App` (que registraria o contexto) também é compilado sob
/// `ssr`.
pub struct PendingSession {
    pub room: String,
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

/// Retira a sessão pendente somente se ela for para a sala pedida — evita
/// que uma sala criada e depois abandonada (ex.: o usuário voltou pra `/` e
/// criou outra) vaze pra uma `RoomPage` diferente.
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
