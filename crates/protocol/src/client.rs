use serde::{Deserialize, Serialize};

use crate::media::QualityLevel;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    /// `password: None` creates a room anyone with the link can join.
    CreateRoom {
        nick: String,
        password: Option<String>,
        room_name: String,
        color: String,
        device_id: String,
    },
    /// `password: None` is only accepted if the room itself has none set.
    JoinRoom {
        room: String,
        nick: String,
        password: Option<String>,
        color: String,
        device_id: String,
    },
    StartShare,
    StopShare,
    WatchShare {
        sharer_id: String,
    },
    StopWatching {
        sharer_id: String,
    },
    /// Answered immediately with `Pong`, so the client can time the round
    /// trip itself — see `ReportLatency`.
    Ping,
    /// The client's own measurement of the `Ping`/`Pong` round trip it just
    /// timed, handed back so the server can broadcast it to the room as
    /// that peer's ping (see `ServerMessage::PeerLatency`).
    ReportLatency {
        ms: u32,
    },
    Offer {
        to: String,
        sdp: String,
    },
    Answer {
        to: String,
        sdp: String,
    },
    IceCandidate {
        to: String,
        stream_owner: String,
        candidate: String,
        sdp_mid: Option<String>,
        sdp_m_line_index: Option<u16>,
    },
    /// Sent by a viewer to change the quality of the stream they're
    /// watching — only the sharer's `RTCRtpSender` for that one connection
    /// can actually apply it, so this is relayed to them rather than
    /// handled server-side (same shape as `Offer`/`Answer`).
    SetQuality {
        to: String,
        quality: QualityLevel,
    },
}
