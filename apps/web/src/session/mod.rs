pub mod audio;
pub mod audio_health;
pub mod handler;
pub mod latency;
pub mod media;
pub mod quality;
pub mod reconnect;
pub(crate) mod share_effects;
pub(crate) mod sharing_state;
pub(crate) mod state;
pub mod video_mode;

#[cfg(feature = "hydrate")]
pub(crate) use sharing_state::SharingState;
pub(crate) use state::RoomState;

use leptos::prelude::*;

/// One member of a room, as the roster UI needs it. `sharing` is never
/// `true` on the local member's own card.
#[derive(Clone, PartialEq)]
pub struct RoomMember {
    pub peer_id: String,
    pub nick: String,
    pub color: String,
    pub sharing: bool,
}

/// The single native-"Stop sharing" (`onended`) listener for the local
/// capture, held so it can be dropped on teardown instead of leaked.
#[cfg(feature = "hydrate")]
pub(crate) type LocalCaptureCallback =
    std::rc::Rc<std::cell::RefCell<Option<wasm_bindgen::prelude::Closure<dyn FnMut()>>>>;

#[cfg(feature = "hydrate")]
#[derive(Clone)]
pub struct RoomSession {
    /// Boxed behind `SignalingTransport` (not the concrete `WsClient`) so
    /// a test can swap in a fake that just records what was sent.
    pub(crate) ws: std::rc::Rc<
        std::cell::RefCell<
            Option<Box<dyn crate::client::seam::signaling_transport::SignalingTransport>>,
        >,
    >,
    pub(crate) outgoing: std::rc::Rc<
        std::cell::RefCell<std::collections::HashMap<String, web_sys::RtcPeerConnection>>,
    >,
    pub(crate) incoming: std::rc::Rc<
        std::cell::RefCell<std::collections::HashMap<String, web_sys::RtcPeerConnection>>,
    >,
    // The JS event callbacks bound to each `outgoing` / `incoming` peer
    // connection (see `handler::PeerCallbacks`), kept alive here rather
    // than `Closure::forget`'d so they — and the `RoomSession` clone one
    // of them captures — are dropped when the connection is removed or the
    // room page unmounts. Keyed the same as the maps above.
    pub(crate) outgoing_callbacks: std::rc::Rc<
        std::cell::RefCell<
            std::collections::HashMap<String, crate::session::handler::PeerCallbacks>,
        >,
    >,
    pub(crate) incoming_callbacks: std::rc::Rc<
        std::cell::RefCell<
            std::collections::HashMap<String, crate::session::handler::PeerCallbacks>,
        >,
    >,
    /// Whether we're sharing and, if so, the captured stream — see
    /// `SharingState` for why this isn't a bare `Option<MediaStream>`.
    pub(crate) sharing: std::rc::Rc<std::cell::RefCell<SharingState>>,
    // The `onended` listener wired to the local capture's first track (the
    // browser's own "Stop sharing" control). Only one local capture exists
    // at a time, so this is a single slot rather than a map. Kept here
    // instead of `Closure::forget`'d so it — and the `RoomSession` clone it
    // captures — is freed on share teardown / source switch, not leaked
    // once per share (finding F08a; also unblocks the F01 `Rc` cycle).
    pub(crate) local_capture_callback: LocalCaptureCallback,
    // Set before an intentional close; `on_close` (async, runs afterwards)
    // checks this flag so it doesn't overwrite the status already set with
    // the generic "connection lost" error.
    pub(crate) expected_close: std::rc::Rc<std::cell::Cell<bool>>,
    // `performance.now()` timestamp of the last `Ping` sent (see
    // `latency.rs`), so the `Pong` handler in `message_handler.rs` can time
    // the round trip. `None` once the matching `Pong` has been handled.
    pub(crate) last_ping_sent_at: std::rc::Rc<std::cell::Cell<Option<f64>>>,
    // Viewer peer_id -> that viewer's running Auto quality poll (see
    // `quality.rs`), so switching them to a fixed tier, them leaving, or
    // the room page unmounting can `clearInterval` it (and drop its
    // closure) instead of leaving it running against a sender that's gone.
    pub(crate) quality_auto_intervals: std::rc::Rc<
        std::cell::RefCell<std::collections::HashMap<String, crate::session::quality::AutoPoll>>,
    >,
    // `true` from the moment an unexpected socket close starts a reconnect
    // until the rejoin's `Joined` snapshot lands (or we give up). Guards
    // against stacking two reconnect loops, and tells the `Joined` handler
    // to replay this member's share/watch intent rather than treat it as a
    // first join. See `session::reconnect`.
    pub(crate) reconnecting: std::rc::Rc<std::cell::Cell<bool>>,
    pub(crate) backoff: std::rc::Rc<std::cell::RefCell<crate::session::reconnect::BackoffPolicy>>,
}

#[cfg(feature = "hydrate")]
impl RoomSession {
    pub(crate) fn new() -> Self {
        Self {
            ws: Default::default(),
            outgoing: Default::default(),
            incoming: Default::default(),
            outgoing_callbacks: Default::default(),
            incoming_callbacks: Default::default(),
            sharing: Default::default(),
            local_capture_callback: Default::default(),
            expected_close: Default::default(),
            last_ping_sent_at: Default::default(),
            quality_auto_intervals: Default::default(),
            reconnecting: Default::default(),
            backoff: Default::default(),
        }
    }
}

#[cfg(not(feature = "hydrate"))]
#[derive(Clone)]
pub(crate) struct RoomSession;

#[cfg(not(feature = "hydrate"))]
impl RoomSession {
    pub(crate) fn new() -> Self {
        Self
    }
}

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
    use crate::client::socket::WsClient;
    use crate::client::storage::{ensure_device_id, save_profile};
    use crate::features::profile::Profile;
    use screen_share_protocol::ClientMessage;

    use crate::session::handler::build_message_handler;

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
                crate::session::reconnect::install_close_handler(
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

    use crate::session::handler::{apply_joined_snapshot, build_message_handler, JoinedSnapshot};

    let Some(mut session) = session::take(&room_code) else {
        return;
    };
    set_requires_password.set(session.requires_password);

    let on_message = build_message_handler(conn.clone(), signals);
    session.ws.set_on_message(on_message);
    crate::session::reconnect::install_close_handler(
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
