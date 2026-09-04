use serde::{Deserialize, Serialize};

use crate::ids::{Color, Nick, PeerId};

/// Maximum members allowed in a single room.
pub const MAX_MEMBERS: usize = 10;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MemberInfo {
    pub peer_id: PeerId,
    pub nick: Nick,
    pub color: Color,
}

/// Who's already watching each active sharer, sent in the join snapshot —
/// avoids waiting for the first `WatchersChanged` to show the right count.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WatcherInfo {
    pub sharer_id: PeerId,
    pub watchers: Vec<PeerId>,
}

/// A member's last-measured round-trip latency to the server, sent in the
/// join snapshot — avoids showing no ping at all until that member's next
/// `Ping`/`Pong` round trip happens to complete.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LatencyInfo {
    pub peer_id: PeerId,
    pub ms: u32,
}

/// Response for `GET /api/rooms/:code` — the minimum the client needs for
/// its dead-link check and to decide whether to show the password field.
///
/// `name` and `member_count` stay in the shape but the server leaves them
/// unset on this unauthenticated endpoint: populated, they would leak the
/// human-chosen room name and occupancy to anyone holding a code. The room
/// name arrives in the `Joined` snapshot instead. Absent fields are
/// omitted from the JSON.
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
