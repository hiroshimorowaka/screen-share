use leptos::prelude::*;

use super::RoomMember;

#[cfg(feature = "hydrate")]
#[derive(Clone)]
pub(super) struct RoomConnection {
    pub(super) ws: std::rc::Rc<std::cell::RefCell<Option<crate::ui::client::socket::WsClient>>>,
    pub(super) outgoing: std::rc::Rc<std::cell::RefCell<std::collections::HashMap<String, web_sys::RtcPeerConnection>>>,
    pub(super) incoming: std::rc::Rc<std::cell::RefCell<std::collections::HashMap<String, web_sys::RtcPeerConnection>>>,
    pub(super) local_stream: std::rc::Rc<std::cell::RefCell<Option<web_sys::MediaStream>>>,
    // Set before an intentional close; `on_close` (async, runs afterwards)
    // checks this flag so it doesn't overwrite the status already set with
    // the generic "connection lost" error.
    pub(super) expected_close: std::rc::Rc<std::cell::Cell<bool>>,
}

#[cfg(feature = "hydrate")]
impl RoomConnection {
    pub(super) fn new() -> Self {
        Self {
            ws: Default::default(),
            outgoing: Default::default(),
            incoming: Default::default(),
            local_stream: Default::default(),
            expected_close: Default::default(),
        }
    }
}

#[cfg(not(feature = "hydrate"))]
#[derive(Clone)]
pub(super) struct RoomConnection;

#[cfg(not(feature = "hydrate"))]
impl RoomConnection {
    pub(super) fn new() -> Self {
        Self
    }
}

#[cfg(not(feature = "hydrate"))]
pub(super) fn setup_room_connection(
    _room_code: String,
    _conn: RoomConnection,
    _set_status: WriteSignal<String>,
    _set_authenticated: WriteSignal<bool>,
    _set_room_name: WriteSignal<Option<String>>,
    _set_members: WriteSignal<Vec<RoomMember>>,
    _set_my_peer_id: WriteSignal<Option<String>>,
    _watching: RwSignal<std::collections::HashSet<String>>,
    _expanded: RwSignal<Option<String>>,
    _watchers_by_sharer: RwSignal<std::collections::HashMap<String, Vec<String>>>,
    _connection_errors: RwSignal<std::collections::HashSet<String>>,
    _set_room_exists: WriteSignal<Option<bool>>,
    _my_peer_id: ReadSignal<Option<String>>,
) -> impl Fn(String, String, String) + Clone + 'static {
    move |_nick: String, _color: String, _password: String| {}
}

#[cfg(feature = "hydrate")]
pub(super) fn setup_room_connection(
    room_code: String,
    conn: RoomConnection,
    set_status: WriteSignal<String>,
    set_authenticated: WriteSignal<bool>,
    set_room_name: WriteSignal<Option<String>>,
    set_members: WriteSignal<Vec<RoomMember>>,
    set_my_peer_id: WriteSignal<Option<String>>,
    watching: RwSignal<std::collections::HashSet<String>>,
    expanded: RwSignal<Option<String>>,
    watchers_by_sharer: RwSignal<std::collections::HashMap<String, Vec<String>>>,
    connection_errors: RwSignal<std::collections::HashSet<String>>,
    set_room_exists: WriteSignal<Option<bool>>,
    my_peer_id: ReadSignal<Option<String>>,
) -> impl Fn(String, String, String) + Clone + 'static {
    use crate::signaling::protocol::ClientMessage;
    use crate::ui::client::socket::WsClient;
    use crate::ui::client::storage::{ensure_device_id, save_profile};
    use crate::ui::profile::Profile;

    use super::message_handler::build_message_handler;

    move |nick: String, color: String, password: String| {
        let conn = conn.clone();
        conn.expected_close.set(false);
        let room_code = room_code.clone();
        set_status.set("Conectando...".to_string());

        let on_message = build_message_handler(set_status, set_authenticated, set_room_name, set_members, set_my_peer_id, conn.clone(), watching, expanded, watchers_by_sharer, connection_errors, set_room_exists, my_peer_id);

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
                ws.on_close({
                    let conn = conn.clone();
                    move || {
                        if !conn.expected_close.get() {
                            set_status.set("Conexão perdida. Recarregue a página para tentar de novo.".to_string());
                        }
                    }
                });
                *conn.ws.borrow_mut() = Some(ws);
                save_profile(&Profile { nick, color });
            }
            Err(_) => set_status.set("Não foi possível conectar ao servidor.".to_string()),
        }
    }
}

#[cfg(not(feature = "hydrate"))]
pub(super) fn adopt_pending_session(
    _room_code: String,
    _conn: RoomConnection,
    _set_status: WriteSignal<String>,
    _set_authenticated: WriteSignal<bool>,
    _set_room_name: WriteSignal<Option<String>>,
    _set_members: WriteSignal<Vec<RoomMember>>,
    _set_my_peer_id: WriteSignal<Option<String>>,
    _watching: RwSignal<std::collections::HashSet<String>>,
    _expanded: RwSignal<Option<String>>,
    _watchers_by_sharer: RwSignal<std::collections::HashMap<String, Vec<String>>>,
    _connection_errors: RwSignal<std::collections::HashSet<String>>,
    _set_room_exists: WriteSignal<Option<bool>>,
    _my_peer_id: ReadSignal<Option<String>>,
) {
}

#[cfg(feature = "hydrate")]
pub(super) fn adopt_pending_session(
    room_code: String,
    conn: RoomConnection,
    set_status: WriteSignal<String>,
    set_authenticated: WriteSignal<bool>,
    set_room_name: WriteSignal<Option<String>>,
    set_members: WriteSignal<Vec<RoomMember>>,
    set_my_peer_id: WriteSignal<Option<String>>,
    watching: RwSignal<std::collections::HashSet<String>>,
    expanded: RwSignal<Option<String>>,
    watchers_by_sharer: RwSignal<std::collections::HashMap<String, Vec<String>>>,
    connection_errors: RwSignal<std::collections::HashSet<String>>,
    set_room_exists: WriteSignal<Option<bool>>,
    my_peer_id: ReadSignal<Option<String>>,
) {
    use crate::ui::client::session;

    use super::message_handler::{apply_joined_snapshot, build_message_handler};

    let Some(mut session) = session::take(&room_code) else { return };

    let on_message = build_message_handler(set_status, set_authenticated, set_room_name, set_members, set_my_peer_id, conn.clone(), watching, expanded, watchers_by_sharer, connection_errors, set_room_exists, my_peer_id);
    session.ws.set_on_message(on_message);
    session.ws.on_close({
        let conn = conn.clone();
        move || {
            if !conn.expected_close.get() {
                set_status.set("Conexão perdida. Recarregue a página para tentar de novo.".to_string());
            }
        }
    });

    // A pending session always comes from the home page creating a new
    // room — it never has a viewer yet.
    apply_joined_snapshot(
        session.room,
        session.room_name,
        session.peer_id,
        session.members,
        session.active_sharers,
        Vec::new(),
        set_my_peer_id,
        set_members,
        set_room_name,
        set_authenticated,
        set_status,
        watchers_by_sharer,
    );

    *conn.ws.borrow_mut() = Some(session.ws);
}
