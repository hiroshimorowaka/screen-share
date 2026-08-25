use serde::{Deserialize, Serialize};

pub const MAX_MEMBERS: usize = 10;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MemberInfo {
    pub peer_id: String,
    pub nick: String,
    pub color: String,
}

/// Who's already watching each active sharer, sent in the join snapshot —
/// avoids waiting for the first `WatchersChanged` to show the right count.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WatcherInfo {
    pub sharer_id: String,
    pub watchers: Vec<String>,
}

/// A member's last-measured round-trip latency to the server, sent in the
/// join snapshot — avoids showing no ping at all until that member's next
/// `Ping`/`Pong` round trip happens to complete.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LatencyInfo {
    pub peer_id: String,
    pub ms: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    /// `password: None` creates a room anyone with the link can join.
    CreateRoom { nick: String, password: Option<String>, room_name: String, color: String, device_id: String },
    /// `password: None` is only accepted if the room itself has none set.
    JoinRoom { room: String, nick: String, password: Option<String>, color: String, device_id: String },
    StartShare,
    StopShare,
    WatchShare { sharer_id: String },
    StopWatching { sharer_id: String },
    /// Answered immediately with `Pong`, so the client can time the round
    /// trip itself — see `ReportLatency`.
    Ping,
    /// The client's own measurement of the `Ping`/`Pong` round trip it just
    /// timed, handed back so the server can broadcast it to the room as
    /// that peer's ping (see `ServerMessage::PeerLatency`).
    ReportLatency { ms: u32 },
    Offer { to: String, sdp: String },
    Answer { to: String, sdp: String },
    IceCandidate {
        to: String,
        stream_owner: String,
        candidate: String,
        sdp_mid: Option<String>,
        sdp_m_line_index: Option<u16>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    Joined {
        peer_id: String,
        room: String,
        room_name: String,
        members: Vec<MemberInfo>,
        active_sharers: Vec<String>,
        watcher_info: Vec<WatcherInfo>,
        latencies: Vec<LatencyInfo>,
    },
    AuthFailed,
    RoomNotFound,
    RoomFull,
    PeerJoined { peer_id: String, nick: String, color: String },
    PeerLeft { peer_id: String },
    /// Sent only to whoever was disconnected by a same-device re-join — never
    /// broadcast; the rest of the room already gets a normal `PeerLeft`.
    Kicked,
    PeerStartedSharing { peer_id: String },
    PeerStoppedSharing { peer_id: String },
    WatchRequested { from: String },
    WatchStopped { from: String },
    /// Broadcast to the whole room, not just the sharer — any card shows
    /// "N watching" from any member's point of view.
    WatchersChanged { sharer_id: String, watchers: Vec<String> },
    Pong,
    /// Broadcast to the whole room — any card can show that peer's ping,
    /// not just the peer who measured it.
    PeerLatency { peer_id: String, ms: u32 },
    Offer { from: String, sdp: String },
    Answer { from: String, sdp: String },
    IceCandidate {
        from: String,
        stream_owner: String,
        candidate: String,
        sdp_mid: Option<String>,
        sdp_m_line_index: Option<u16>,
    },
}

/// Response for `GET /api/rooms/:code`. `name`/`member_count`/
/// `requires_password` are omitted from the JSON when `exists` is `false`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RoomStatus {
    pub exists: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub member_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requires_password: Option<bool>,
}

#[cfg(test)]
mod tests {
    use super::*;

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
            watcher_info: vec![WatcherInfo { sharer_id: "peer-1".to_string(), watchers: vec!["peer-2".to_string()] }],
            latencies: vec![LatencyInfo { peer_id: "peer-1".to_string(), ms: 42 }],
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
        let msg = ServerMessage::PeerLatency { peer_id: "peer-1".to_string(), ms: 87 };
        let json = serde_json::to_string(&msg).unwrap();
        assert_eq!(json, r#"{"type":"peer_latency","peer_id":"peer-1","ms":87}"#);

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
        let msg = ClientMessage::WatchShare { sharer_id: "peer-1".to_string() };
        let json = serde_json::to_string(&msg).unwrap();
        assert_eq!(json, r#"{"type":"watch_share","sharer_id":"peer-1"}"#);

        let parsed: ClientMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, msg);
    }

    #[test]
    fn room_status_omits_absent_fields_when_room_does_not_exist() {
        let status = RoomStatus { exists: false, name: None, member_count: None, requires_password: None };
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
}
