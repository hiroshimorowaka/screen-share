use serde::{Deserialize, Serialize};

use crate::info::{LatencyInfo, MemberInfo, WatcherInfo};
use crate::media::{QualityLevel, TurnCredentials};

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
        /// `None` when no TURN server is configured for this deployment —
        /// callers fall back to STUN-only ICE in that case.
        turn: Option<TurnCredentials>,
    },
    AuthFailed,
    RoomNotFound,
    RoomFull,
    /// This connection already created or joined a room. Signaling state
    /// is bound to the connection, so a second `CreateRoom`/`JoinRoom` on
    /// the same socket is refused rather than silently leaking the first
    /// room's membership — a new room needs a new connection.
    AlreadyInRoom,
    /// The server is at its room capacity — the global cap or this
    /// client's per-client cap (see `MAX_ROOMS` / `MAX_ROOMS_PER_CLIENT`
    /// in `signaling::registry`). Retrying immediately won't help.
    ServerAtCapacity,
    /// A `CreateRoom`/`JoinRoom` field failed validation — nick or room
    /// name empty, too long, or carrying control / bidi characters, or an
    /// unknown colour id (see `protocol::validate`).
    InvalidInput,
    /// Too many wrong-password attempts against this room recently — see
    /// `MAX_PASSWORD_ATTEMPTS` in `registry.rs`. Sent instead of
    /// `AuthFailed` even if the password given this time was correct, so a
    /// successful guess after brute-forcing gains nothing.
    TooManyAttempts,
    PeerJoined {
        peer_id: String,
        nick: String,
        color: String,
    },
    PeerLeft {
        peer_id: String,
    },
    /// Sent only to whoever was disconnected by a same-device re-join — never
    /// broadcast; the rest of the room already gets a normal `PeerLeft`.
    Kicked,
    PeerStartedSharing {
        peer_id: String,
    },
    PeerStoppedSharing {
        peer_id: String,
    },
    WatchRequested {
        from: String,
    },
    WatchStopped {
        from: String,
    },
    /// Broadcast to the whole room, not just the sharer — any card shows
    /// "N watching" from any member's point of view.
    WatchersChanged {
        sharer_id: String,
        watchers: Vec<String>,
    },
    Pong,
    /// Broadcast to the whole room — any card can show that peer's ping,
    /// not just the peer who measured it.
    PeerLatency {
        peer_id: String,
        ms: u32,
    },
    Offer {
        from: String,
        sdp: String,
    },
    Answer {
        from: String,
        sdp: String,
    },
    IceCandidate {
        from: String,
        stream_owner: String,
        candidate: String,
        sdp_mid: Option<String>,
        sdp_m_line_index: Option<u16>,
    },
    QualityRequested {
        from: String,
        quality: QualityLevel,
    },
}
