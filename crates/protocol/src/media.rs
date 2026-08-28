use serde::{Deserialize, Serialize};

/// A short-lived TURN credential, minted server-side (see
/// `signaling::turn`) and handed to a member only once they've actually
/// authenticated into a room — never exposed through an unauthenticated
/// endpoint, since a TURN relay is bandwidth anyone with the credential can
/// spend. `username` embeds its own expiry, so the client never needs to
/// know the TTL separately; it just holds onto these for the life of the
/// WebSocket connection and reuses them for every peer connection it opens.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TurnCredentials {
    pub urls: Vec<String>,
    pub username: String,
    pub password: String,
}

/// A viewer's chosen quality for one sharer's stream — set independently
/// per (sharer, viewer) pair, since each is its own P2P connection. `Auto`
/// hands control to the sharer's own bandwidth-adaptive monitor instead of
/// pinning a fixed tier; see `ui::pages::room::quality`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum QualityLevel {
    Auto,
    High,
    Medium,
    Low,
}
