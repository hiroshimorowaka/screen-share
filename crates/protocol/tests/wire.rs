//! Wire-protocol round-trip tests. Moved out of src/lib.rs (Phase 4)
//! so the type definitions stay uncluttered.

use screen_share_protocol::*;

#[test]
fn create_room_message_round_trips_through_json() {
    let msg = ClientMessage::CreateRoom {
        nick: "Ana".to_string(),
        password: Some("abacate".to_string()),
        room_name: "Sala dos lindos".to_string(),
        color: "coral".to_string(),
        device_id: "device-1".to_string(),
    };
    let json = serde_json::to_string(&msg).unwrap();
    assert_eq!(
        json,
        r#"{"type":"create_room","nick":"Ana","password":"abacate","room_name":"Sala dos lindos","color":"coral","device_id":"device-1"}"#
    );

    let parsed: ClientMessage = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, msg);
}

#[test]
fn create_room_message_without_password_round_trips_through_json() {
    let msg = ClientMessage::CreateRoom {
        nick: "Ana".to_string(),
        password: None,
        room_name: "Sala dos lindos".to_string(),
        color: "coral".to_string(),
        device_id: "device-1".to_string(),
    };
    let json = serde_json::to_string(&msg).unwrap();
    assert_eq!(
        json,
        r#"{"type":"create_room","nick":"Ana","password":null,"room_name":"Sala dos lindos","color":"coral","device_id":"device-1"}"#
    );

    let parsed: ClientMessage = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, msg);
}

#[test]
fn join_room_message_round_trips_through_json() {
    let msg = ClientMessage::JoinRoom {
        room: "ABCD1234".to_string(),
        nick: "Bia".to_string(),
        password: Some("abacate".to_string()),
        color: "sky".to_string(),
        device_id: "device-2".to_string(),
    };
    let json = serde_json::to_string(&msg).unwrap();
    let parsed: ClientMessage = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, msg);
}

#[test]
fn joined_server_message_round_trips_through_json() {
    let msg = ServerMessage::Joined {
        peer_id: "peer-1".to_string(),
        room: "ABCD1234".to_string(),
        room_name: "Sala dos lindos".to_string(),
        members: vec![MemberInfo {
            peer_id: "peer-1".to_string(),
            nick: "Ana".to_string(),
            color: "coral".to_string(),
        }],
        active_sharers: vec![],
        watcher_info: vec![WatcherInfo {
            sharer_id: "peer-1".to_string(),
            watchers: vec!["peer-2".to_string()],
        }],
        latencies: vec![LatencyInfo {
            peer_id: "peer-1".to_string(),
            ms: 42,
        }],
        turn: Some(TurnCredentials {
            urls: vec!["turn:example.com:3478".to_string()],
            username: "1234567890".to_string(),
            password: "s3cr3t-hash".to_string(),
        }),
    };
    let json = serde_json::to_string(&msg).unwrap();
    let parsed: ServerMessage = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, msg);
}

#[test]
fn ping_message_round_trips_through_json() {
    let msg = ClientMessage::Ping;
    let json = serde_json::to_string(&msg).unwrap();
    assert_eq!(json, r#"{"type":"ping"}"#);

    let parsed: ClientMessage = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, msg);
}

#[test]
fn report_latency_message_round_trips_through_json() {
    let msg = ClientMessage::ReportLatency { ms: 87 };
    let json = serde_json::to_string(&msg).unwrap();
    assert_eq!(json, r#"{"type":"report_latency","ms":87}"#);

    let parsed: ClientMessage = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, msg);
}

#[test]
fn peer_latency_message_round_trips_through_json() {
    let msg = ServerMessage::PeerLatency {
        peer_id: "peer-1".to_string(),
        ms: 87,
    };
    let json = serde_json::to_string(&msg).unwrap();
    assert_eq!(
        json,
        r#"{"type":"peer_latency","peer_id":"peer-1","ms":87}"#
    );

    let parsed: ServerMessage = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, msg);
}

#[test]
fn watchers_changed_message_round_trips_through_json() {
    let msg = ServerMessage::WatchersChanged {
        sharer_id: "peer-1".to_string(),
        watchers: vec!["peer-2".to_string(), "peer-3".to_string()],
    };
    let json = serde_json::to_string(&msg).unwrap();
    assert_eq!(
        json,
        r#"{"type":"watchers_changed","sharer_id":"peer-1","watchers":["peer-2","peer-3"]}"#
    );

    let parsed: ServerMessage = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, msg);
}

#[test]
fn kicked_message_round_trips_through_json() {
    let msg = ServerMessage::Kicked;
    let json = serde_json::to_string(&msg).unwrap();
    assert_eq!(json, r#"{"type":"kicked"}"#);

    let parsed: ServerMessage = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, msg);
}

#[test]
fn watch_share_message_round_trips_through_json() {
    let msg = ClientMessage::WatchShare {
        sharer_id: "peer-1".to_string(),
    };
    let json = serde_json::to_string(&msg).unwrap();
    assert_eq!(json, r#"{"type":"watch_share","sharer_id":"peer-1"}"#);

    let parsed: ClientMessage = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, msg);
}

#[test]
fn room_status_omits_absent_fields_when_room_does_not_exist() {
    let status = RoomStatus {
        exists: false,
        name: None,
        member_count: None,
        requires_password: None,
    };
    let json = serde_json::to_string(&status).unwrap();
    assert_eq!(json, r#"{"exists":false}"#);

    let parsed: RoomStatus = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, status);
}

#[test]
fn ice_candidate_carries_stream_owner() {
    let msg = ClientMessage::IceCandidate {
        to: "peer-2".to_string(),
        stream_owner: "peer-1".to_string(),
        candidate: "candidate-data".to_string(),
        sdp_mid: None,
        sdp_m_line_index: None,
    };
    let json = serde_json::to_string(&msg).unwrap();
    assert!(json.contains(r#""stream_owner":"peer-1""#));

    let parsed: ClientMessage = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, msg);
}

#[test]
fn set_quality_message_round_trips_through_json() {
    let msg = ClientMessage::SetQuality {
        to: "peer-1".to_string(),
        quality: QualityLevel::Medium,
    };
    let json = serde_json::to_string(&msg).unwrap();
    assert_eq!(
        json,
        r#"{"type":"set_quality","to":"peer-1","quality":"medium"}"#
    );

    let parsed: ClientMessage = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, msg);
}

#[test]
fn quality_requested_message_round_trips_through_json() {
    let msg = ServerMessage::QualityRequested {
        from: "peer-2".to_string(),
        quality: QualityLevel::Auto,
    };
    let json = serde_json::to_string(&msg).unwrap();
    assert_eq!(
        json,
        r#"{"type":"quality_requested","from":"peer-2","quality":"auto"}"#
    );

    let parsed: ServerMessage = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, msg);
}
