//! Automatic reconnection after the signaling WebSocket drops.
//!
//! An unexpected close schedules a rejoin: reopen the socket, send
//! `JoinRoom` again with the same nick/colour/password, and once the room
//! snapshot comes back, replay what this member was doing (sharing, and/or
//! watching specific people). Without it, a brief network blip on the
//! sharer's or a viewer's side ends the room session with no recovery
//! short of a manual page reload.
//!
//! The peer identity is *not* preserved across a reconnect — the rejoin
//! gets a fresh `peer_id`. Other members see this member leave and rejoin;
//! anyone who was watching this member has to click watch again. Restoring
//! that too would need server-side session resumption, which is out of
//! scope here.
//!
//! [`BackoffPolicy`] below is the pure, browser-free core (how long to wait
//! before each attempt, and when to give up); the rest of the module is the
//! `hydrate`-only wiring that drives it.

pub use screen_share_domain::backoff::BackoffPolicy;

#[cfg(all(test, target_arch = "wasm32", feature = "hydrate"))]
#[path = "wasm_tests.rs"]
mod wasm_tests;

#[cfg(not(feature = "hydrate"))]
pub(crate) fn drop_peers_on_cleanup(_conn: super::RoomSession) {}

#[cfg(feature = "hydrate")]
pub(crate) use wiring::{
    drop_peers_on_cleanup, install_close_handler, replay_intent_after_rejoin, teardown_session,
};

#[cfg(feature = "hydrate")]
mod wiring {
    use leptos::prelude::*;
    use wasm_bindgen::JsCast;

    use super::BackoffPolicy;
    use crate::client::socket::WsClient;
    use crate::client::storage::{ensure_device_id, load_room_session};
    use crate::room::messages::build_message_handler;
    use crate::room::{RoomSession, RoomState};
    use screen_share_protocol::ClientMessage;

    /// Installs an `on_close` handler on `ws` that reconnects on an
    /// unexpected drop and does nothing on a deliberate one
    /// (`conn.expected_close`). Used by every place that opens the room
    /// socket — the first connect, the pending-session adopt, and each
    /// reconnect attempt itself. `room_code` is all it needs: the
    /// nick/colour/password to rejoin with are read back from the same
    /// `localStorage` entry the manual-reload path already relies on.
    pub(crate) fn install_close_handler(
        ws: &WsClient,
        conn: RoomSession,
        signals: RoomState,
        room_code: String,
    ) {
        ws.on_close(move || {
            if conn.expected_close.get() {
                return;
            }
            begin_reconnect(conn.clone(), signals, room_code.clone());
        });
    }

    /// Tears down the dead peer connections and schedules the first rejoin
    /// attempt.
    fn begin_reconnect(conn: RoomSession, signals: RoomState, room_code: String) {
        if conn.reconnecting.replace(true) {
            // A reconnect is already in flight (e.g. a second close event
            // for the same drop) — don't stack a second schedule.
            return;
        }
        conn.backoff.borrow_mut().reset();
        drop_all_peer_connections(&conn);
        schedule_attempt(conn, signals, room_code);
    }

    /// Closes and forgets every peer connection and its bookkeeping — the
    /// Auto-quality polls, the connection maps, and the event callbacks.
    /// Used by a reconnect (dead connections) and by `RoomPage`'s cleanup
    /// when the user leaves the room.
    pub(crate) fn drop_all_peer_connections(conn: &RoomSession) {
        crate::room::quality::stop_all_auto_polling(conn);
        conn.incoming_streams.borrow_mut().clear();
        for (_, link) in conn.links_out.borrow_mut().drain() {
            link.pc.close();
        }
        for (_, link) in conn.links_in.borrow_mut().drain() {
            link.pc.close();
        }
    }

    /// Shuts a room session down without navigating: mark the close
    /// expected, stop any reconnect loop, tear every peer connection down,
    /// and take the `WsClient` out of its `RefCell` and close it.
    ///
    /// Taking the socket *out* is the point: it breaks the `Rc` cycle
    /// `conn.ws` -> `WsClient`'s message closure -> a `RoomSession` clone
    /// -> `conn.ws`, so the session (open socket, maps, timers) is actually
    /// freed. Otherwise a non-button exit (browser back, `navigate`, an SPA
    /// route change) leaves the socket open with `expected_close` false, so
    /// the server's idle reap trips `on_close` and the reconnect loop
    /// rejoins the room on a page the user already left.
    ///
    /// Idempotent: safe to call from both the explicit "leave" path and
    /// the `on_cleanup` that also fires on unmount.
    pub(crate) fn teardown_session(conn: &RoomSession) {
        conn.expected_close.set(true);
        conn.reconnecting.set(false);
        drop_all_peer_connections(conn);
        let ws = conn.ws.borrow_mut().take();
        if let Some(ws) = ws {
            ws.close();
        }
    }

    /// Registers an `on_cleanup` on the current owner (`RoomPage`) that
    /// runs [`teardown_session`] when the room page unmounts — the only
    /// teardown for every exit that isn't the "leave" button (browser
    /// back, `navigate`, an SPA route change). Without it the message
    /// closure — which holds a `RoomSession` clone — is never dropped and
    /// keeps the whole session (socket, connections, timers) alive.
    pub(crate) fn drop_peers_on_cleanup(conn: RoomSession) {
        // `on_cleanup` is `Send + Sync`-bound; `RoomSession` holds `Rc`s.
        let conn = send_wrapper::SendWrapper::new(conn);
        on_cleanup(move || teardown_session(&conn));
    }

    fn schedule_attempt(conn: RoomSession, signals: RoomState, room_code: String) {
        let jitter = js_sys::Math::random();
        let Some(delay_ms) = conn.backoff.borrow_mut().next_delay_ms(jitter) else {
            conn.reconnecting.set(false);
            signals
                .set_status
                .set("Conexão perdida. Recarregue a página para tentar de novo.".to_string());
            return;
        };

        let attempt = conn.backoff.borrow().attempts_made();
        signals.set_status.set(format!(
            "Reconectando... (tentativa {attempt} de {})",
            BackoffPolicy::max_attempts()
        ));

        let Some(window) = web_sys::window() else {
            return;
        };
        let on_tick = wasm_bindgen::prelude::Closure::once_into_js(move || {
            open_socket(conn, signals, room_code);
        });
        let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(
            on_tick.as_ref().unchecked_ref(),
            delay_ms as i32,
        );
    }

    fn open_socket(conn: RoomSession, signals: RoomState, room_code: String) {
        // A deliberate leave between the schedule and this tick clears the
        // flag — nothing to do then.
        if !conn.reconnecting.get() {
            return;
        }
        let Some(creds) = load_room_session(&room_code) else {
            // No stored credentials to rejoin with (private window, cleared
            // storage) — a silent reconnect isn't possible.
            conn.reconnecting.set(false);
            signals
                .set_status
                .set("Conexão perdida. Recarregue a página para tentar de novo.".to_string());
            return;
        };

        let on_message = build_message_handler(conn.clone(), signals);
        match WsClient::connect("/ws", on_message) {
            Ok(ws) => {
                ws.on_open({
                    let conn = conn.clone();
                    let room_code = room_code.clone();
                    move || {
                        if let Some(ws) = conn.ws.borrow().as_ref() {
                            ws.send(&ClientMessage::JoinRoom {
                                room: room_code.clone(),
                                nick: creds.nick.clone(),
                                password: creds.password.clone(),
                                color: creds.color.clone(),
                                device_id: ensure_device_id(),
                            });
                        }
                    }
                });
                install_close_handler(&ws, conn.clone(), signals, room_code);
                *conn.ws.borrow_mut() = Some(Box::new(ws));
            }
            // Couldn't even open the socket — treat it as another failed
            // attempt and back off again.
            Err(_) => schedule_attempt(conn, signals, room_code),
        }
    }

    /// Called from the `Joined` handler once a reconnect's rejoin lands:
    /// clears the reconnecting flag, resets the backoff, and re-asserts
    /// this member's intent — restart the share if one was running, and
    /// re-watch anyone still in the room this member was watching before.
    pub(crate) fn replay_intent_after_rejoin(
        conn: &RoomSession,
        signals: RoomState,
        present_peer_ids: &[String],
    ) {
        if !conn.reconnecting.replace(false) {
            return;
        }
        conn.backoff.borrow_mut().reset();

        let ws_borrow = conn.ws.borrow();
        let Some(ws) = ws_borrow.as_ref() else {
            return;
        };

        if conn.sharing.borrow().is_sharing() {
            ws.send(&ClientMessage::StartShare);
        }
        for sharer_id in signals.watching.get_untracked() {
            if present_peer_ids.iter().any(|id| id == &sharer_id) {
                ws.send(&ClientMessage::WatchShare { sharer_id });
            }
        }
    }
}
