//! The client↔server signaling wire protocol.
//!
//! One definition of each message shape, shared verbatim by the browser
//! (`apps/web`, WASM) and the relay server (`crates/signaling`). The
//! server routes these; it has no opinion about what they mean — that
//! lives in `apps/web/src/session`.

mod client;
pub mod ids;
mod info;
mod media;
mod server;
pub mod validate;

pub use client::ClientMessage;
pub use ids::{Color, IdError, Nick, PeerId, RoomCode};
pub use info::{LatencyInfo, MemberInfo, RoomStatus, WatcherInfo, MAX_MEMBERS};
pub use media::{QualityLevel, TurnCredentials};
pub use server::ServerMessage;
