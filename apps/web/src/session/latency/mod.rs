//! Self-measured ping to the signaling server: each client times its own
//! `Ping`/`Pong` round trip and reports the result (`ClientMessage::
//! ReportLatency`) so the server can broadcast it to the room. See
//! `Registry::report_latency` for the server side and `message_handler.rs`
//! for where the `Pong`/`PeerLatency` replies are handled.

/// How often each client re-measures and reports its own ping — frequent
/// enough that the badge feels live, infrequent enough not to spam the
/// signaling socket.
#[cfg(feature = "hydrate")]
const PING_INTERVAL_MS: i32 = 5_000;

#[cfg(not(feature = "hydrate"))]
pub(crate) fn setup_ping_loop(_conn: crate::session::RoomSession) {}

#[cfg(feature = "hydrate")]
fn send_ping(conn: &crate::session::RoomSession) {
    use screen_share_protocol::ClientMessage;

    let Some(window) = web_sys::window() else {
        return;
    };
    let Some(performance) = window.performance() else {
        return;
    };
    let ws_borrow = conn.ws.borrow();
    let Some(ws) = ws_borrow.as_ref() else { return };
    conn.last_ping_sent_at.set(Some(performance.now()));
    ws.send(&ClientMessage::Ping);
}

/// Elapsed time since a `performance.now()` timestamp, in whole
/// milliseconds — used to time a `Ping`/`Pong` round trip once the `Pong`
/// arrives.
#[cfg(feature = "hydrate")]
pub(crate) fn round_trip_ms_since(sent_at: f64) -> Option<u32> {
    let performance = web_sys::window()?.performance()?;
    let elapsed = performance.now() - sent_at;
    Some(elapsed.max(0.0).round() as u32)
}

/// Periodically times a `Ping`/`Pong` round trip to the signaling server —
/// nothing before the room is actually joined (`conn.ws` unset yet, or the
/// connection dropped) sends anything, `send_ping` just no-ops.
#[cfg(feature = "hydrate")]
pub(crate) fn setup_ping_loop(conn: crate::session::RoomSession) {
    use wasm_bindgen::prelude::Closure;

    send_ping(&conn); // first reading without waiting a full interval

    // Cleared when `RoomPage` is disposed. Without this the tick keeps
    // firing after the user leaves the room, calling `WsClient::send` on
    // an already-closed socket ("WebSocket is already in CLOSING or
    // CLOSED state").
    let on_tick = Closure::<dyn FnMut()>::new(move || send_ping(&conn));
    crate::client::dom::interval_until_cleanup(on_tick, PING_INTERVAL_MS);
}

#[cfg(all(test, target_arch = "wasm32", feature = "hydrate"))]
#[path = "wasm_tests.rs"]
mod wasm_tests;
