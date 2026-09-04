//! Re-export of the wire identifier newtypes (`screen_share_protocol::ids`)
//! so callers that already depend on `domain` — but not directly on
//! `protocol` — can name them without a second dependency line.

pub use screen_share_protocol::ids::{Color, IdError, Nick, PeerId, RoomCode};
