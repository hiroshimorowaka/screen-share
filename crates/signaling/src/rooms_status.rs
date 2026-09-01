use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::extract::{ConnectInfo, Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use tokio::time::Instant;

use super::handshake::HandshakeConfig;
use super::registry::Registry;
use screen_share_protocol::RoomStatus;

/// Per-client request budget for the unauthenticated room-status endpoint.
/// Generous for a human (the home page checks a handful of remembered
/// rooms on load, the room page one) but enough that the endpoint can't
/// be used as a fast room-code enumeration oracle.
const MAX_ROOM_STATUS_REQUESTS: usize = 30;
const ROOM_STATUS_WINDOW: Duration = Duration::from_secs(10);

/// Cap on how many distinct client keys the limiter tracks before it
/// sweeps out the ones whose window has fully elapsed — bounds memory
/// against a churn of one-off source IPs.
const MAX_TRACKED_CLIENTS: usize = 10_000;

/// Sliding-window per-client rate limiter for [`room_status_handler`].
/// Same shape as the wrong-password guard in `registry`, kept separate
/// because this one is process-wide rather than per-room.
#[derive(Clone, Default)]
pub struct RoomStatusLimiter {
    hits: Arc<Mutex<HashMap<String, Vec<Instant>>>>,
}

impl RoomStatusLimiter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Records a request from `key` and reports whether it stays within
    /// [`MAX_ROOM_STATUS_REQUESTS`] over [`ROOM_STATUS_WINDOW`].
    fn allow(&self, key: &str) -> bool {
        let now = Instant::now();
        let mut hits = self
            .hits
            .lock()
            .expect("room-status limiter mutex should never be poisoned");

        if hits.len() > MAX_TRACKED_CLIENTS {
            hits.retain(|_, stamps| {
                stamps.retain(|&t| now.duration_since(t) < ROOM_STATUS_WINDOW);
                !stamps.is_empty()
            });
        }

        let bucket = hits.entry(key.to_string()).or_default();
        bucket.retain(|&t| now.duration_since(t) < ROOM_STATUS_WINDOW);
        bucket.push(now);
        bucket.len() <= MAX_ROOM_STATUS_REQUESTS
    }
}

/// `GET /api/rooms/:code` — just enough for the client's dead-link check
/// and to decide whether to show the password field.
///
/// Deliberately does **not** return the human-chosen room name or the
/// member count to this unauthenticated, membership-unchecked endpoint
/// (finding F06): those are an information leak to anyone holding a code
/// and make enumeration observable. They're delivered in the `Joined`
/// snapshot once a client is actually in the room.
pub async fn room_status_handler(
    State(registry): State<Registry>,
    State(limiter): State<RoomStatusLimiter>,
    State(handshake): State<HandshakeConfig>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Path(code): Path<String>,
) -> Result<Json<RoomStatus>, StatusCode> {
    if !limiter.allow(&handshake.client_key(&headers, peer)) {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }

    let status = match registry.room_status(&code) {
        Some(summary) => RoomStatus {
            exists: true,
            name: None,
            member_count: None,
            requires_password: Some(summary.requires_password),
        },
        None => RoomStatus {
            exists: false,
            name: None,
            member_count: None,
            requires_password: None,
        },
    };
    Ok(Json(status))
}

#[cfg(test)]
#[path = "rooms_status_tests.rs"]
mod tests;
