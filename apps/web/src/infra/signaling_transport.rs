//! The seam between session/room code and the concrete `WsClient`
//! (`infra::socket`) — step 8 of the structure-refactor plan.
//! `RoomSession.ws` holds a `Box<dyn SignalingTransport>` instead of a
//! `WsClient` directly, so a test can stand in a fake that just records
//! what was sent, instead of opening a real `WsClient::connect` socket
//! against a live server. Only `send`/`close` are on the trait — the
//! rest of `WsClient`'s API (`on_open`, `on_close`, `set_on_message`) is
//! only ever called on a freshly connected, still-concrete `WsClient`
//! before it's stored here, so abstracting it would add surface no
//! caller uses.

#[cfg(feature = "hydrate")]
use screen_share_protocol::ClientMessage;

#[cfg(feature = "hydrate")]
pub(crate) trait SignalingTransport {
    fn send(&self, msg: &ClientMessage);
    fn close(&self);
}

#[cfg(feature = "hydrate")]
impl SignalingTransport for crate::infra::socket::WsClient {
    fn send(&self, msg: &ClientMessage) {
        Self::send(self, msg)
    }

    fn close(&self) {
        Self::close(self)
    }
}
