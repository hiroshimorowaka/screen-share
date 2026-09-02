//! Browser (`wasm32`) tests for `reconnect::teardown_session` — it needs a
//! real `WsClient` (a browser `WebSocket`), so it can't run in the native
//! suite. Split out so `.cargo/mutants.toml`'s `**/*_wasm_tests.rs`
//! exclusion covers it.

use wasm_bindgen_test::*;

use super::*;
use crate::infra::socket::WsClient;
use crate::session::RoomSession;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
fn teardown_session_breaks_the_socket_cycle_and_stops_the_reconnect_loop() {
    let conn = RoomSession::new();

    // A live socket plus an in-flight reconnect — the state a non-button
    // exit used to leave behind (finding F01).
    let ws = WsClient::connect("/ws", |_| {}).expect("WebSocket constructs in the test browser");
    *conn.ws.borrow_mut() = Some(ws);
    conn.reconnecting.set(true);
    conn.expected_close.set(false);

    teardown_session(&conn);

    assert!(
        conn.ws.borrow().is_none(),
        "the socket is taken out of its RefCell so the Rc cycle can't keep the session alive"
    );
    assert!(
        conn.expected_close.get(),
        "the close is marked expected so on_close won't start a rejoin"
    );
    assert!(
        !conn.reconnecting.get(),
        "any scheduled reconnect attempt early-returns"
    );

    // Idempotent: the on_cleanup path calls it again on unmount.
    teardown_session(&conn);
    assert!(conn.ws.borrow().is_none());
}
