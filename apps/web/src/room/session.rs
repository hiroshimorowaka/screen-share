//! `RoomSession` — the per-tab imperative handle the room's runtime code
//! (message handler, media, reconnect) shares: the live socket, the peer
//! connections and their callbacks, and the local capture state. Kept
//! non-reactive `Rc<RefCell<…>>` on purpose so it stays callable from JS
//! callbacks and from a bare `wasm-bindgen-test` with no reactive runtime.

/// The single native-"Stop sharing" (`onended`) listener for the local
/// capture, held so it can be dropped on teardown instead of leaked.
#[cfg(feature = "hydrate")]
pub(crate) type LocalCaptureCallback =
    std::rc::Rc<std::cell::RefCell<Option<wasm_bindgen::prelude::Closure<dyn FnMut()>>>>;

#[cfg(feature = "hydrate")]
#[derive(Clone)]
pub struct RoomSession {
    /// Boxed behind `SignalingTransport` (not the concrete `WsClient`) so
    /// a test can swap in a fake that just records what was sent.
    pub(crate) ws: std::rc::Rc<
        std::cell::RefCell<
            Option<Box<dyn crate::client::seam::signaling_transport::SignalingTransport>>,
        >,
    >,
    pub(crate) outgoing: std::rc::Rc<
        std::cell::RefCell<std::collections::HashMap<String, web_sys::RtcPeerConnection>>,
    >,
    pub(crate) incoming: std::rc::Rc<
        std::cell::RefCell<std::collections::HashMap<String, web_sys::RtcPeerConnection>>,
    >,
    // The JS event callbacks bound to each `outgoing` / `incoming` peer
    // connection (see `messages::PeerCallbacks`), kept alive here rather
    // than `Closure::forget`'d so they — and the `RoomSession` clone one
    // of them captures — are dropped when the connection is removed or the
    // room page unmounts. Keyed the same as the maps above.
    pub(crate) outgoing_callbacks: std::rc::Rc<
        std::cell::RefCell<std::collections::HashMap<String, crate::room::messages::PeerCallbacks>>,
    >,
    pub(crate) incoming_callbacks: std::rc::Rc<
        std::cell::RefCell<std::collections::HashMap<String, crate::room::messages::PeerCallbacks>>,
    >,
    /// Whether we're sharing and, if so, the captured stream — see
    /// `SharingState` for why this isn't a bare `Option<MediaStream>`.
    pub(crate) sharing: std::rc::Rc<std::cell::RefCell<super::SharingState>>,
    // The `onended` listener wired to the local capture's first track (the
    // browser's own "Stop sharing" control). Only one local capture exists
    // at a time, so this is a single slot rather than a map. Kept here
    // instead of `Closure::forget`'d so it — and the `RoomSession` clone it
    // captures — is freed on share teardown / source switch, not leaked
    // once per share (finding F08a; also unblocks the F01 `Rc` cycle).
    pub(crate) local_capture_callback: LocalCaptureCallback,
    // Set before an intentional close; `on_close` (async, runs afterwards)
    // checks this flag so it doesn't overwrite the status already set with
    // the generic "connection lost" error.
    pub(crate) expected_close: std::rc::Rc<std::cell::Cell<bool>>,
    // `performance.now()` timestamp of the last `Ping` sent (see
    // `latency.rs`), so the `Pong` handler in `messages` can time the round
    // trip. `None` once the matching `Pong` has been handled.
    pub(crate) last_ping_sent_at: std::rc::Rc<std::cell::Cell<Option<f64>>>,
    // Viewer peer_id -> that viewer's running Auto quality poll (see
    // `quality.rs`), so switching them to a fixed tier, them leaving, or
    // the room page unmounting can `clearInterval` it (and drop its
    // closure) instead of leaving it running against a sender that's gone.
    pub(crate) quality_auto_intervals: std::rc::Rc<
        std::cell::RefCell<std::collections::HashMap<String, crate::room::quality::AutoPoll>>,
    >,
    // `true` from the moment an unexpected socket close starts a reconnect
    // until the rejoin's `Joined` snapshot lands (or we give up). Guards
    // against stacking two reconnect loops, and tells the `Joined` handler
    // to replay this member's share/watch intent rather than treat it as a
    // first join. See `room::reconnect`.
    pub(crate) reconnecting: std::rc::Rc<std::cell::Cell<bool>>,
    pub(crate) backoff: std::rc::Rc<std::cell::RefCell<crate::room::reconnect::BackoffPolicy>>,
}

#[cfg(feature = "hydrate")]
impl RoomSession {
    pub(crate) fn new() -> Self {
        Self {
            ws: Default::default(),
            outgoing: Default::default(),
            incoming: Default::default(),
            outgoing_callbacks: Default::default(),
            incoming_callbacks: Default::default(),
            sharing: Default::default(),
            local_capture_callback: Default::default(),
            expected_close: Default::default(),
            last_ping_sent_at: Default::default(),
            quality_auto_intervals: Default::default(),
            reconnecting: Default::default(),
            backoff: Default::default(),
        }
    }
}

#[cfg(not(feature = "hydrate"))]
#[derive(Clone)]
pub(crate) struct RoomSession;

#[cfg(not(feature = "hydrate"))]
impl RoomSession {
    pub(crate) fn new() -> Self {
        Self
    }
}
