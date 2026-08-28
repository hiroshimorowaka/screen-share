//! The signaling relay server: an in-memory room registry plus the Axum
//! WebSocket and HTTP handlers that drive it.
//!
//! Intentionally dumb about WebRTC — it routes `screen_share_protocol`
//! messages between named peers and owns only what needs a trusted shared
//! vantage point (membership, the member cap, password hashing and
//! rate limiting, roster/sharer/watcher fan-out, TURN credential minting).

pub mod auth;
pub mod registry;
pub mod rooms_status;
pub mod state;
pub mod turn;
pub mod ws;
