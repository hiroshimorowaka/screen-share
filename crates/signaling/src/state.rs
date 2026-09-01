use super::handshake::HandshakeConfig;
use super::registry::Registry;
use super::rooms_status::RoomStatusLimiter;
use super::turn::TurnConfig;

/// Router state for the signaling endpoints. Each field is extracted
/// independently via `FromRef` below, so a handler that only needs one of
/// them (like `room_status_handler`) keeps declaring just `State<Registry>`
/// and never sees the others.
#[derive(Clone)]
pub struct SignalingState {
    pub registry: Registry,
    pub turn: Option<TurnConfig>,
    pub handshake: HandshakeConfig,
    pub room_status_limiter: RoomStatusLimiter,
}

impl axum::extract::FromRef<SignalingState> for Registry {
    fn from_ref(state: &SignalingState) -> Self {
        state.registry.clone()
    }
}

impl axum::extract::FromRef<SignalingState> for Option<TurnConfig> {
    fn from_ref(state: &SignalingState) -> Self {
        state.turn.clone()
    }
}

impl axum::extract::FromRef<SignalingState> for HandshakeConfig {
    fn from_ref(state: &SignalingState) -> Self {
        state.handshake.clone()
    }
}

impl axum::extract::FromRef<SignalingState> for RoomStatusLimiter {
    fn from_ref(state: &SignalingState) -> Self {
        state.room_status_limiter.clone()
    }
}
