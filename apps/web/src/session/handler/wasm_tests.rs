//! Browser (`wasm32`) tests for `apply_joined_snapshot` — the single
//! point where the server's `Joined` snapshot is fanned out into the
//! room UI's reactive signals. It calls `save_recent_room` (localStorage)
//! so it can only run in a browser.

use std::collections::{HashMap, HashSet};

use screen_share_protocol::{
    Color, LatencyInfo, MemberInfo, Nick, PeerId, TurnCredentials, WatcherInfo,
};
use wasm_bindgen_test::*;

use super::*;
use crate::client::storage::load_recent_rooms;
use crate::session::{RoomMember, RoomSignals};

wasm_bindgen_test_configure!(run_in_browser);

/// Read handles for the signals `apply_joined_snapshot` writes.
struct Reads {
    my_peer_id: ReadSignal<Option<String>>,
    members: ReadSignal<Vec<RoomMember>>,
    room_name: ReadSignal<Option<String>>,
    authenticated: ReadSignal<bool>,
    status: ReadSignal<String>,
    watchers_by_sharer: RwSignal<HashMap<String, Vec<String>>>,
    latency_by_peer: RwSignal<HashMap<String, u32>>,
    turn_credentials: RwSignal<Option<TurnCredentials>>,
}

/// A full `RoomSignals` plus handles to read back the ones under test.
/// Must be called inside a reactive `Owner` (`signal`/`RwSignal` panic
/// without one).
fn fresh_signals() -> (RoomSignals, Reads) {
    let (status, set_status) = signal(String::new());
    let (authenticated, set_authenticated) = signal(false);
    let (room_name, set_room_name) = signal(None::<String>);
    let (members, set_members) = signal(Vec::<RoomMember>::new());
    let (my_peer_id, set_my_peer_id) = signal(None::<String>);
    let (_room_exists, set_room_exists) = signal(None::<bool>);
    let watching = RwSignal::new(HashSet::<String>::new());
    let expanded = RwSignal::new(None::<String>);
    let watchers_by_sharer = RwSignal::new(HashMap::<String, Vec<String>>::new());
    let connection_errors = RwSignal::new(HashSet::<String>::new());
    let latency_by_peer = RwSignal::new(HashMap::<String, u32>::new());
    let turn_credentials = RwSignal::new(None::<TurnCredentials>);
    let audio_preset = RwSignal::new(crate::session::audio::AudioPreset::default());
    let video_mode = RwSignal::new(crate::session::video_mode::VideoMode::default());

    let signals = RoomSignals {
        set_status,
        set_authenticated,
        set_room_name,
        set_members,
        set_my_peer_id,
        my_peer_id,
        set_room_exists,
        watching,
        expanded,
        watchers_by_sharer,
        connection_errors,
        latency_by_peer,
        turn_credentials,
        audio_preset,
        video_mode,
    };
    let reads = Reads {
        my_peer_id,
        members,
        room_name,
        authenticated,
        status,
        watchers_by_sharer,
        latency_by_peer,
        turn_credentials,
    };
    (signals, reads)
}

fn member(peer_id: &str, nick: &str) -> MemberInfo {
    MemberInfo {
        peer_id: PeerId::from_relay(peer_id),
        nick: Nick::from_relay(nick),
        color: Color::from_relay("coral"),
    }
}

fn snapshot(room_code: &str, room_name: &str, peer_id: &str) -> JoinedSnapshot {
    JoinedSnapshot {
        room_code: room_code.to_string(),
        room_name: room_name.to_string(),
        peer_id: peer_id.to_string(),
        members: vec![member(peer_id, "Ana")],
        active_sharers: vec![],
        watcher_info: vec![],
        latencies: vec![],
        turn: None,
    }
}

#[wasm_bindgen_test]
fn derives_each_members_sharing_flag_from_active_sharers() {
    Owner::new().with(|| {
        let (signals, reads) = fresh_signals();
        apply_joined_snapshot(
            JoinedSnapshot {
                members: vec![
                    member("me", "Ana"),
                    member("p2", "Bia"),
                    member("p3", "Caio"),
                ],
                active_sharers: vec!["p2".to_string()],
                ..snapshot("ROOM1", "Sala", "me")
            },
            signals,
        );

        let members = reads.members.get_untracked();
        assert_eq!(members.len(), 3);
        let sharing: Vec<&str> = members
            .iter()
            .filter(|m| m.sharing)
            .map(|m| m.peer_id.as_str())
            .collect();
        assert_eq!(sharing, vec!["p2"], "only the active sharer is marked");
    });
}

#[wasm_bindgen_test]
fn maps_watchers_latencies_and_turn_into_their_signals() {
    Owner::new().with(|| {
        let (signals, reads) = fresh_signals();
        let turn = TurnCredentials {
            urls: vec!["turn:relay.example:3478".to_string()],
            username: "user".to_string(),
            password: "pass".to_string(),
        };
        apply_joined_snapshot(
            JoinedSnapshot {
                members: vec![member("me", "Ana"), member("p2", "Bia")],
                active_sharers: vec!["p2".to_string()],
                watcher_info: vec![WatcherInfo {
                    sharer_id: PeerId::from_relay("p2"),
                    watchers: vec![PeerId::from_relay("me")],
                }],
                latencies: vec![LatencyInfo {
                    peer_id: PeerId::from_relay("p2"),
                    ms: 42,
                }],
                turn: Some(turn.clone()),
                ..snapshot("ROOM2", "Sala 2", "me")
            },
            signals,
        );

        assert_eq!(
            reads.watchers_by_sharer.get_untracked().get("p2"),
            Some(&vec!["me".to_string()])
        );
        assert_eq!(reads.latency_by_peer.get_untracked().get("p2"), Some(&42));
        assert_eq!(reads.turn_credentials.get_untracked(), Some(turn));
    });
}

#[wasm_bindgen_test]
fn marks_the_client_authenticated_and_records_its_identity_and_status() {
    Owner::new().with(|| {
        let (signals, reads) = fresh_signals();
        apply_joined_snapshot(snapshot("ROOM3", "Sala 3", "my-peer"), signals);

        assert!(reads.authenticated.get_untracked(), "join gate must clear");
        assert_eq!(reads.my_peer_id.get_untracked().as_deref(), Some("my-peer"));
        assert_eq!(reads.room_name.get_untracked().as_deref(), Some("Sala 3"));
        assert_eq!(reads.status.get_untracked(), "Conectado.");
    });
}

#[wasm_bindgen_test]
fn persists_the_room_to_the_recent_rooms_list() {
    if let Some(storage) = web_sys::window().unwrap().local_storage().unwrap() {
        let _ = storage.clear();
    }

    Owner::new().with(|| {
        let (signals, _reads) = fresh_signals();
        apply_joined_snapshot(snapshot("RECENT1", "Minha sala", "me"), signals);
    });

    let recent = load_recent_rooms();
    assert!(
        recent
            .iter()
            .any(|r| r.code == "RECENT1" && r.name == "Minha sala"),
        "the joined room should be saved as a recent room, got {recent:?}"
    );
}
