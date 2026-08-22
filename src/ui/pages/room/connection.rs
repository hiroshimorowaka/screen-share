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

/// The reactive state a WebSocket message can update, bundled so the
/// connection-setup and message-routing functions each take one argument
/// for it instead of the same eleven signals apiece.
///
/// Every field is read from `#[cfg(feature = "hydrate")]` code only — the
/// `ssr` build only ever passes this struct through inert stub functions,
/// so an `ssr`-only compile sees no reads and would otherwise flag it as
/// dead code.
#[cfg_attr(not(feature = "hydrate"), allow(dead_code))]
#[derive(Clone, Copy)]
pub(super) struct RoomSignals {
    pub(super) set_status: WriteSignal<String>,
    pub(super) set_authenticated: WriteSignal<bool>,
    pub(super) set_room_name: WriteSignal<Option<String>>,
    pub(super) set_members: WriteSignal<Vec<RoomMember>>,
    pub(super) set_my_peer_id: WriteSignal<Option<String>>,
    pub(super) my_peer_id: ReadSignal<Option<String>>,
    pub(super) set_room_exists: WriteSignal<Option<bool>>,
    pub(super) watching: RwSignal<std::collections::HashSet<String>>,
    pub(super) expanded: RwSignal<Option<String>>,
    pub(super) watchers_by_sharer: RwSignal<std::collections::HashMap<String, Vec<String>>>,
    pub(super) connection_errors: RwSignal<std::collections::HashSet<String>>,
}

#[cfg(not(feature = "hydrate"))]
pub(super) fn setup_room_connection(
    _room_code: String,
    _conn: RoomConnection,
    _signals: RoomSignals,
) -> impl Fn(String, String, String) + Clone + 'static {
    move |_nick: String, _color: String, _password: String| {}
}

#[cfg(feature = "hydrate")]
pub(super) fn setup_room_connection(
    room_code: String,
    conn: RoomConnection,
    signals: RoomSignals,
) -> impl Fn(String, String, String) + Clone + 'static {
    use crate::signaling::protocol::ClientMessage;
    use crate::ui::client::socket::WsClient;
    use crate::ui::client::storage::{ensure_device_id, save_profile};
    use crate::ui::profile::Profile;

    use super::message_handler::build_message_handler;

    let RoomSignals { set_status, .. } = signals;

    move |nick: String, color: String, password: String| {
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
pub(super) fn adopt_pending_session(_room_code: String, _conn: RoomConnection, _signals: RoomSignals) {}

#[cfg(feature = "hydrate")]
pub(super) fn adopt_pending_session(room_code: String, conn: RoomConnection, signals: RoomSignals) {
    use crate::ui::client::session;

    use super::message_handler::{apply_joined_snapshot, build_message_handler};

    let RoomSignals { set_status, .. } = signals;

    let Some(mut session) = session::take(&room_code) else { return };

    let on_message = build_message_handler(conn.clone(), signals);
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
        signals,
    );

    *conn.ws.borrow_mut() = Some(session.ws);
}
