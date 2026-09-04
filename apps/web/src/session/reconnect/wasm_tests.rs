//! Browser (`wasm32`) tests for `reconnect`. `teardown_session` needs a
//! real `WsClient` (a browser `WebSocket`); `replay_intent_after_rejoin`
//! only needs *something* behind `SignalingTransport`, so it runs
//! against a `FakeTransport` instead — no live socket, no signaling
//! server. Split out so `.cargo/mutants.toml`'s `**/*_wasm_tests.rs`
//! exclusion covers it.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use leptos::prelude::*;
use screen_share_protocol::ClientMessage;
use wasm_bindgen_test::*;

use super::*;
use crate::client::signaling_transport::SignalingTransport;
use crate::client::socket::WsClient;
use crate::session::{RoomSession, RoomSignals, SharingState};

wasm_bindgen_test_configure!(run_in_browser);

/// A `SignalingTransport` that records every sent message instead of
/// opening a real `WebSocket` — see the module doc comment.
struct FakeTransport {
    sent: Rc<RefCell<Vec<ClientMessage>>>,
}

impl SignalingTransport for FakeTransport {
    fn send(&self, msg: &ClientMessage) {
        self.sent.borrow_mut().push(msg.clone());
    }

    fn close(&self) {}
}

/// A minimal `RoomSignals` for tests that only read `watching` — the
/// rest of the struct is wired to throwaway signals so the type
/// constructs at all. Must run inside an `Owner` (`signal`/`RwSignal`
/// panic without one).
fn signals_watching(watching: RwSignal<HashSet<String>>) -> RoomSignals {
    let (_status, set_status) = signal(String::new());
    let (_authenticated, set_authenticated) = signal(false);
    let (_room_name, set_room_name) = signal(None::<String>);
    let (_members, set_members) = signal(Vec::new());
    let (_my_peer_id, set_my_peer_id) = signal(None::<String>);
    let (my_peer_id, _) = signal(None::<String>);
    let (_room_exists, set_room_exists) = signal(None::<bool>);

    RoomSignals {
        set_status,
        set_authenticated,
        set_room_name,
        set_members,
        set_my_peer_id,
        my_peer_id,
        set_room_exists,
        watching,
        expanded: RwSignal::new(None),
        watchers_by_sharer: RwSignal::new(HashMap::new()),
        connection_errors: RwSignal::new(HashSet::new()),
        latency_by_peer: RwSignal::new(HashMap::new()),
        turn_credentials: RwSignal::new(None),
        audio_preset: RwSignal::new(crate::session::audio::AudioPreset::default()),
        video_mode: RwSignal::new(crate::session::video_mode::VideoMode::default()),
    }
}

#[wasm_bindgen_test]
fn teardown_session_breaks_the_socket_cycle_and_stops_the_reconnect_loop() {
    let conn = RoomSession::new();

    // A live socket plus an in-flight reconnect — the state a non-button
    // exit used to leave behind (finding F01).
    let ws = WsClient::connect("/ws", |_| {}).expect("WebSocket constructs in the test browser");
    *conn.ws.borrow_mut() = Some(Box::new(ws));
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

fn fake_transport() -> (Box<dyn SignalingTransport>, Rc<RefCell<Vec<ClientMessage>>>) {
    let sent = Rc::new(RefCell::new(Vec::new()));
    let transport = FakeTransport { sent: sent.clone() };
    (Box::new(transport), sent)
}

#[wasm_bindgen_test]
fn replay_intent_after_rejoin_resends_the_share_and_only_present_watches() {
    Owner::new().with(|| {
        let conn = RoomSession::new();
        let (transport, sent) = fake_transport();
        *conn.ws.borrow_mut() = Some(transport);
        *conn.sharing.borrow_mut() = SharingState::Sharing {
            stream: web_sys::MediaStream::new().unwrap(),
        };
        conn.reconnecting.set(true);

        let watching = RwSignal::new(HashSet::from(["p2".to_string(), "p3".to_string()]));
        let signals = signals_watching(watching);

        // "p3" left while we were disconnected — only "p2" is still here.
        replay_intent_after_rejoin(&conn, signals, &["p2".to_string()]);

        assert!(
            !conn.reconnecting.get(),
            "the reconnect latch clears once intent is replayed"
        );
        assert_eq!(
            *sent.borrow(),
            vec![
                ClientMessage::StartShare,
                ClientMessage::WatchShare {
                    sharer_id: "p2".to_string()
                },
            ],
            "the share restarts and only the still-present watch is resent"
        );
    });
}

#[wasm_bindgen_test]
fn replay_intent_after_rejoin_is_a_noop_when_no_reconnect_was_in_flight() {
    Owner::new().with(|| {
        let conn = RoomSession::new();
        let (transport, sent) = fake_transport();
        *conn.ws.borrow_mut() = Some(transport);
        // `reconnecting` was never set — a normal first join, not a rejoin.

        let signals = signals_watching(RwSignal::new(HashSet::from(["p2".to_string()])));
        replay_intent_after_rejoin(&conn, signals, &["p2".to_string()]);

        assert!(
            sent.borrow().is_empty(),
            "nothing is replayed outside an actual reconnect"
        );
    });
}
