//! Opening (and re-adopting) the room's WebSocket: wire a fresh
//! `WsClient` to the message handler and the reconnect close-handler,
//! send the join, and stash the boxed transport on the `RoomSession`.
//! `adopt_pending_session` takes over the socket the home page already
//! opened when it created the room, so the creator skips the nick gate.

use leptos::prelude::*;

use super::{RoomSession, RoomState};

#[cfg(not(feature = "hydrate"))]
pub(crate) fn setup_room_connection(
    _room_code: String,
    _conn: RoomSession,
    _signals: RoomState,
) -> impl Fn(String, String, Option<String>) + Clone + 'static {
    move |_nick: String, _color: String, _password: Option<String>| {}
}

#[cfg(feature = "hydrate")]
pub(crate) fn setup_room_connection(
    room_code: String,
    conn: RoomSession,
    signals: RoomState,
) -> impl Fn(String, String, Option<String>) + Clone + 'static {
    use screen_share_protocol::ClientMessage;

    use crate::client::socket::WsClient;
    use crate::client::storage::{ensure_device_id, save_profile};
    use crate::profile::Profile;
    use crate::room::messages::build_message_handler;

    let RoomState { set_status, .. } = signals;

    move |nick: String, color: String, password: Option<String>| {
        let conn = conn.clone();
        conn.expected_close.set(false);
        let room_code = room_code.clone();
        set_status.set("Conectando...".to_string());

        let on_message = build_message_handler(conn.clone(), signals);

        match WsClient::connect("/ws", on_message) {
            Ok(ws) => {
                ws.on_open({
                    let conn = conn.clone();
                    let room_code = room_code.clone();
                    let nick = nick.clone();
                    let color = color.clone();
                    let password = password.clone();
                    move || {
                        if let Some(ws) = conn.ws.borrow().as_ref() {
                            ws.send(&ClientMessage::JoinRoom {
                                room: room_code.clone(),
                                nick: nick.clone(),
                                password: password.clone(),
                                color: color.clone(),
                                device_id: ensure_device_id(),
                            });
                        }
                    }
                });
                crate::room::reconnect::install_close_handler(
                    &ws,
                    conn.clone(),
                    signals,
                    room_code.clone(),
                );
                *conn.ws.borrow_mut() = Some(Box::new(ws));
                save_profile(&Profile { nick, color });
            }
            Err(_) => set_status.set("Não foi possível conectar ao servidor.".to_string()),
        }
    }
}

#[cfg(not(feature = "hydrate"))]
pub(crate) fn adopt_pending_session(
    _room_code: String,
    _conn: RoomSession,
    _signals: RoomState,
    _set_requires_password: WriteSignal<bool>,
) {
}

#[cfg(feature = "hydrate")]
pub(crate) fn adopt_pending_session(
    room_code: String,
    conn: RoomSession,
    signals: RoomState,
    set_requires_password: WriteSignal<bool>,
) {
    use crate::client::session;
    use crate::room::messages::{apply_joined_snapshot, build_message_handler, JoinedSnapshot};

    let Some(mut session) = session::take(&room_code) else {
        return;
    };
    set_requires_password.set(session.requires_password);

    let on_message = build_message_handler(conn.clone(), signals);
    session.ws.set_on_message(on_message);
    crate::room::reconnect::install_close_handler(
        &session.ws,
        conn.clone(),
        signals,
        room_code.clone(),
    );

    // A pending session always comes from the home page creating a new
    // room — it never has a viewer yet.
    apply_joined_snapshot(
        JoinedSnapshot {
            room_code: session.room,
            room_name: session.room_name,
            peer_id: session.peer_id,
            members: session.members,
            active_sharers: session.active_sharers,
            watcher_info: Vec::new(),
            latencies: Vec::new(),
            turn: session.turn,
        },
        signals,
    );

    *conn.ws.borrow_mut() = Some(Box::new(session.ws));
}
