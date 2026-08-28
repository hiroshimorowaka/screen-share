use serde::{Deserialize, Serialize};

/// Maximum members allowed in a single room.
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
