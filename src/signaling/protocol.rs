use serde::{Deserialize, Serialize};

pub const MAX_MEMBERS: usize = 10;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MemberInfo {
    pub peer_id: String,
    pub nick: String,
    pub color: String,
}

/// Quem está assistindo a transmissão de um membro específico, no momento
/// em que alguém entra numa sala já em andamento — dá pra mostrar a
/// contagem/lista de espectadores de cada compartilhamento ativo sem
/// esperar o primeiro `WatchersChanged` chegar.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WatcherInfo {
    pub sharer_id: String,
    pub watchers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    CreateRoom { nick: String, password: String, room_name: String, color: String, device_id: String },
    JoinRoom { room: String, nick: String, password: String, color: String, device_id: String },
    StartShare,
    StopShare,
    WatchShare { sharer_id: String },
    StopWatching { sharer_id: String },
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
    },
    AuthFailed,
    RoomNotFound,
    RoomFull,
    PeerJoined { peer_id: String, nick: String, color: String },
    PeerLeft { peer_id: String },
    /// Mandado só pra quem foi desconectado porque o mesmo dispositivo
    /// entrou de novo nessa sala (outra aba/janela) — nunca broadcast pro
    /// resto da sala, que já recebe o `PeerLeft` normal dessa saída.
    Kicked,
    PeerStartedSharing { peer_id: String },
    PeerStoppedSharing { peer_id: String },
    WatchRequested { from: String },
    WatchStopped { from: String },
    /// Manda pra sala inteira (quem compartilha e quem assiste), não só
    /// pro dono da transmissão — é o que permite mostrar "N assistindo"
    /// no card de qualquer um, pro ponto de vista de qualquer membro.
    WatchersChanged { sharer_id: String, watchers: Vec<String> },
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

/// Resposta do endpoint `GET /api/rooms/:code` (fora do WebSocket) — permite
/// checar se uma sala existe (e ver seu nome) sem abrir conexão nem digitar
/// senha. Quando `exists` é `false`, os outros dois campos são omitidos do
/// JSON (não seriam relevantes).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RoomStatus {
    pub exists: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub member_count: Option<usize>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_room_message_round_trips_through_json() {
        let msg = ClientMessage::CreateRoom {
            nick: "Ana".to_string(),
            password: "abacate".to_string(),
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
    fn join_room_message_round_trips_through_json() {
        let msg = ClientMessage::JoinRoom {
            room: "ABCD1234".to_string(),
            nick: "Bia".to_string(),
            password: "abacate".to_string(),
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
        };
        let json = serde_json::to_string(&msg).unwrap();
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
        let status = RoomStatus { exists: false, name: None, member_count: None };
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
