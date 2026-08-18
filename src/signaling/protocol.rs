use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    CreateRoom,
    Join { room: String },
    Offer { to: String, sdp: String },
    Answer { to: String, sdp: String },
    IceCandidate {
        to: String,
        candidate: String,
        sdp_mid: Option<String>,
        sdp_m_line_index: Option<u16>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    RoomCreated { room: String, peer_id: String },
    Joined { peer_id: String },
    RoomNotFound,
    PeerJoined { peer_id: String },
    PeerLeft { peer_id: String },
    RoomClosed,
    Offer { from: String, sdp: String },
    Answer { from: String, sdp: String },
    IceCandidate {
        from: String,
        candidate: String,
        sdp_mid: Option<String>,
        sdp_m_line_index: Option<u16>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn join_message_round_trips_through_json() {
        let msg = ClientMessage::Join { room: "ABCD1234".to_string() };
        let json = serde_json::to_string(&msg).unwrap();
        assert_eq!(json, r#"{"type":"join","room":"ABCD1234"}"#);

        let parsed: ClientMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, msg);
    }

    #[test]
    fn offer_server_message_round_trips_through_json() {
        let msg = ServerMessage::Offer { from: "peer-1".to_string(), sdp: "v=0...".to_string() };
        let json = serde_json::to_string(&msg).unwrap();
        assert_eq!(json, r#"{"type":"offer","from":"peer-1","sdp":"v=0..."}"#);

        let parsed: ServerMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, msg);
    }
}
