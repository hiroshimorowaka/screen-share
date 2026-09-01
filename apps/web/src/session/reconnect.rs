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

/// First retry waits about this long. Short — most drops are brief.
const BASE_DELAY_MS: u32 = 1_000;
/// Backoff never waits longer than this between attempts.
const MAX_DELAY_MS: u32 = 20_000;
/// Give up after this many failed attempts (~1-2 min of trying with the
/// delays above) and fall back to asking the user to reload.
const MAX_ATTEMPTS: u32 = 8;

/// Exponential backoff with "half jitter" and a hard attempt cap. Pure and
/// deterministic given the jitter fraction, so the schedule is unit-tested
/// directly.
#[derive(Debug, Default)]
pub struct BackoffPolicy {
    attempts_made: u32,
}

impl BackoffPolicy {
    pub fn new() -> Self {
        Self::default()
    }

    /// The delay before the next attempt, or `None` once [`MAX_ATTEMPTS`]
    /// have been used. Advances the attempt counter.
    ///
    /// `jitter01` is a caller-supplied random fraction in `[0.0, 1.0)`
    /// (injected rather than sampled here to keep this testable). The
    /// returned delay lies in `[target/2, target)` where `target` is the
    /// uncapped-then-capped exponential value — spreading retries out so a
    /// whole room that dropped together doesn't reconnect in lockstep.
    pub fn next_delay_ms(&mut self, jitter01: f64) -> Option<u32> {
        if self.attempts_made >= MAX_ATTEMPTS {
            return None;
        }
        let exponential = BASE_DELAY_MS.saturating_mul(1u32 << self.attempts_made);
        let target = exponential.min(MAX_DELAY_MS);
        self.attempts_made += 1;

        let half = target / 2;
        let jittered = half as f64 + half as f64 * jitter01.clamp(0.0, 1.0);
        Some(jittered as u32)
    }

    /// Resets the counter after a reconnection succeeds, so a later drop
    /// starts its backoff from scratch.
    pub fn reset(&mut self) {
        self.attempts_made = 0;
    }

    pub fn attempts_made(&self) -> u32 {
        self.attempts_made
    }

    /// Total attempts the policy will make before giving up — for the
    /// "attempt N of M" status text.
    pub fn max_attempts() -> u32 {
        MAX_ATTEMPTS
    }
}

#[cfg(test)]
#[path = "reconnect_tests.rs"]
mod tests;

#[cfg(feature = "hydrate")]
pub(crate) use wiring::{install_close_handler, replay_intent_after_rejoin};

#[cfg(feature = "hydrate")]
mod wiring {
    use leptos::prelude::*;
    use wasm_bindgen::JsCast;

    use super::BackoffPolicy;
    use crate::infra::socket::WsClient;
    use crate::infra::storage::{ensure_device_id, load_room_session};
    use crate::session::handler::build_message_handler;
    use crate::session::{RoomSession, RoomSignals};
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
        signals: RoomSignals,
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
    fn begin_reconnect(conn: RoomSession, signals: RoomSignals, room_code: String) {
        if conn.reconnecting.replace(true) {
            // A reconnect is already in flight (e.g. a second close event
            // for the same drop) — don't stack a second schedule.
            return;
        }
        conn.backoff.borrow_mut().reset();
        drop_all_peer_connections(&conn);
        schedule_attempt(conn, signals, room_code);
    }

    fn drop_all_peer_connections(conn: &RoomSession) {
        // A `for … in conn.…borrow().keys()…` holds the borrow across the
        // whole loop, and `stop_auto_polling` takes `borrow_mut()` — bind
        // the keys to a `let` so the read borrow is released first.
        let auto_poll_viewers: Vec<String> = conn
            .quality_auto_intervals
            .borrow()
            .keys()
            .cloned()
            .collect();
        for viewer_peer_id in auto_poll_viewers {
            crate::session::quality::stop_auto_polling(conn, &viewer_peer_id);
        }
        for (_, pc) in conn.outgoing.borrow_mut().drain() {
            pc.close();
        }
        for (_, pc) in conn.incoming.borrow_mut().drain() {
            pc.close();
        }
    }

    fn schedule_attempt(conn: RoomSession, signals: RoomSignals, room_code: String) {
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

    fn open_socket(conn: RoomSession, signals: RoomSignals, room_code: String) {
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
                *conn.ws.borrow_mut() = Some(ws);
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
        signals: RoomSignals,
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

        if conn.local_stream.borrow().is_some() {
            ws.send(&ClientMessage::StartShare);
        }
        for sharer_id in signals.watching.get_untracked() {
            if present_peer_ids.iter().any(|id| id == &sharer_id) {
                ws.send(&ClientMessage::WatchShare { sharer_id });
            }
        }
    }
}
