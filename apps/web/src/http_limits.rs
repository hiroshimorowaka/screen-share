//! DoS guards for the HTTP (render / server-fn) routes (P3 follow-up:
//! slow-HTTP and render-amplification had no ceiling). Kept out of
//! `main.rs` so the wiring is testable.
//!
//! Applied to the Leptos routes only — the caller merges the signaling
//! router (`/ws`, `/api/rooms/:code`) in afterwards, so a long-lived
//! WebSocket is never subject to the per-request timeout and the status
//! endpoint keeps its own rate limiter.
//!
//! Two pieces, wired separately because they need different things from
//! the caller: [`apply`] adds the body / concurrency / timeout tower
//! layers (no state), and [`apply_rate_limit`] adds the per-client
//! request-rate cap (needs `ConnectInfo` and the per-deployment
//! [`HandshakeConfig`]). Re-audit follow-up A-05: [`apply`]'s concurrency
//! ceiling is global, so a single source could still monopolise it —
//! [`apply_rate_limit`] bounds one client first.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::extract::{ConnectInfo, DefaultBodyLimit, Request, State};
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::Router;
use screen_share_signaling::handshake::HandshakeConfig;
use tower::limit::GlobalConcurrencyLimitLayer;
use tower_http::timeout::TimeoutLayer;

/// Longest an HTTP request handler may run before the client gets a 408.
/// Generous for a cold SSR render on the small VM, tight enough to shed a
/// slow-loris / render-amplification pile-up instead of pinning a worker.
pub const HTTP_REQUEST_TIMEOUT: Duration = Duration::from_secs(20);

/// Largest HTTP request body accepted. Every request this app makes over
/// HTTP is tiny (a create-room form POST, a status GET); signaling rides a
/// WebSocket, not a body. Well under axum's 2 MiB default.
pub const MAX_HTTP_BODY_BYTES: usize = 64 * 1024;

/// Ceiling on render / server-fn calls in flight at once. Excess requests
/// queue (and 408 via [`HTTP_REQUEST_TIMEOUT`] if the flood persists)
/// rather than each allocating an unbounded render on the 256 MB VM.
pub const MAX_CONCURRENT_HTTP_REQUESTS: usize = 64;

/// Per-client request budget for the render / static-asset routes over
/// [`SSR_RATE_WINDOW`]. A cold page load is a handful of requests (the
/// HTML, the wasm-bindgen JS + `.wasm`, the stylesheet, the favicon), so
/// this still allows many full loads per window — no human reaches it —
/// while capping one source hammering the per-request SSR render on the
/// 256 MB VM. Deliberately looser than the WS and `/api/rooms/:code`
/// budgets: those guard a single small endpoint each, this guards every
/// page and asset.
pub const MAX_SSR_REQUESTS_PER_WINDOW: usize = 240;

/// Sliding window for [`MAX_SSR_REQUESTS_PER_WINDOW`].
pub const SSR_RATE_WINDOW: Duration = Duration::from_secs(10);

/// Cap on distinct client keys the limiter tracks before it sweeps out
/// the ones whose window has fully elapsed — bounds memory against a
/// churn of one-off source IPs. Mirrors `RoomStatusLimiter`.
const MAX_TRACKED_SSR_CLIENTS: usize = 10_000;

/// Process-wide sliding-window per-client rate limiter for the SSR
/// routes. Same shape as `RoomStatusLimiter` in `signaling`, kept here
/// because it guards this crate's render surface.
#[derive(Clone, Default)]
pub struct SsrRateLimiter {
    hits: Arc<Mutex<HashMap<String, Vec<Instant>>>>,
}

impl SsrRateLimiter {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Records a request from `key` and reports whether it stays within
    /// [`MAX_SSR_REQUESTS_PER_WINDOW`] over [`SSR_RATE_WINDOW`].
    fn allow(&self, key: &str) -> bool {
        let now = Instant::now();
        let mut hits = self
            .hits
            .lock()
            .expect("SSR rate-limiter mutex should never be poisoned");

        if hits.len() > MAX_TRACKED_SSR_CLIENTS {
            hits.retain(|_, stamps| {
                stamps.retain(|&t| now.duration_since(t) < SSR_RATE_WINDOW);
                !stamps.is_empty()
            });
        }

        let bucket = hits.entry(key.to_string()).or_default();
        bucket.retain(|&t| now.duration_since(t) < SSR_RATE_WINDOW);
        bucket.push(now);
        bucket.len() <= MAX_SSR_REQUESTS_PER_WINDOW
    }
}

/// State for [`rate_limit`]: the shared limiter plus the deployment's
/// [`HandshakeConfig`], so the SSR cap is keyed by the *same* client
/// identity as the WebSocket and room-status limiters (the forwarded
/// client IP behind a trusted proxy, the real TCP peer otherwise).
#[derive(Clone)]
pub struct SsrRateLimit {
    limiter: SsrRateLimiter,
    handshake: HandshakeConfig,
}

impl SsrRateLimit {
    #[must_use]
    pub fn new(handshake: HandshakeConfig) -> Self {
        Self {
            limiter: SsrRateLimiter::new(),
            handshake,
        }
    }
}

/// `axum::middleware::from_fn_with_state` handler: reject with `429` once
/// a client is over [`MAX_SSR_REQUESTS_PER_WINDOW`] for the window, before
/// the request takes a concurrency permit or starts a render.
pub async fn rate_limit(
    State(state): State<SsrRateLimit>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    request: Request,
    next: Next,
) -> Response {
    let key = state.handshake.client_key(request.headers(), peer);
    if !state.limiter.allow(&key) {
        return StatusCode::TOO_MANY_REQUESTS.into_response();
    }
    next.run(request).await
}

/// Wraps `router` with, innermost to outermost: the body cap, the
/// concurrency ceiling, then the per-request timeout (which also bounds
/// time spent queued behind the ceiling).
pub fn apply<S>(router: Router<S>) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    router
        .layer(DefaultBodyLimit::max(MAX_HTTP_BODY_BYTES))
        .layer(GlobalConcurrencyLimitLayer::new(
            MAX_CONCURRENT_HTTP_REQUESTS,
        ))
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            HTTP_REQUEST_TIMEOUT,
        ))
}

/// Wraps `router` with the per-client [`rate_limit`] middleware. Separate
/// from [`apply`] so the caller keeps ordering it outermost — a flood is
/// shed before it can occupy one of [`apply`]'s concurrency permits — and
/// because it needs `ConnectInfo` plus per-deployment state that the
/// tower layers in [`apply`] do not.
pub fn apply_rate_limit<S>(router: Router<S>, state: SsrRateLimit) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    router.layer(axum::middleware::from_fn_with_state(state, rate_limit))
}
