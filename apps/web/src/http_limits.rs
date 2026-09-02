//! DoS guards for the HTTP (render / server-fn) routes (P3 follow-up:
//! slow-HTTP and render-amplification had no ceiling). Kept out of
//! `main.rs` so the wiring is testable.
//!
//! Applied to the Leptos routes only — the caller merges the signaling
//! router (`/ws`, `/api/rooms/:code`) in afterwards, so a long-lived
//! WebSocket is never subject to the per-request timeout and the status
//! endpoint keeps its own rate limiter.

use std::time::Duration;

use axum::extract::DefaultBodyLimit;
use axum::http::StatusCode;
use axum::Router;
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
