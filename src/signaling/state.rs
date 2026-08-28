use super::registry::Registry;
use super::turn::TurnConfig;

/// Router state for the signaling endpoints. `Registry` and `Option<TurnConfig>`
/// are extracted independently via `FromRef` below, so handlers that only
/// need one of them (like `room_status_handler`) keep declaring
/// `State<Registry>` and never see the other.
#[derive(Clone)]
pub struct SignalingState {
    pub registry: Registry,
    pub turn: Option<TurnConfig>,
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
