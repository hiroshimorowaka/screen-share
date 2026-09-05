//! Composes the full Axum service: the Leptos SSR routes (each wrapped to
//! re-publish the per-request CSP nonce), the signaling relay (`/ws`,
//! `/api/rooms/{code}`), and the DoS guards. Merge order matters — see
//! the inline comments below.

use axum::routing::get;
use axum::Router;
use leptos::prelude::LeptosOptions;
use leptos_axum::{generate_route_list, LeptosRoutes};
use screen_share::app::{shell, App};
use screen_share_signaling::handshake::HandshakeConfig;
use screen_share_signaling::rooms_status::room_status_handler;
use screen_share_signaling::state::SignalingState;
use screen_share_signaling::ws::ws_handler;

use crate::middleware::{limits, security};

/// Builds the complete service. `dev_csp` is `true` in a non-production
/// (`cargo leptos watch`) run, where the live-reload WebSocket needs a
/// looser policy. `handshake` keys the SSR rate limiter by the same
/// client identity the WebSocket and room-status limiters use.
pub fn build(
    leptos_options: LeptosOptions,
    signaling_state: SignalingState,
    handshake: HandshakeConfig,
    dev_csp: bool,
) -> Router {
    let routes = generate_route_list(App);

    let signaling_router = Router::new()
        .route("/ws", get(ws_handler))
        .route("/api/rooms/{code}", get(room_status_handler))
        .with_state(signaling_state);

    let leptos_router = Router::new()
        // `_with_context(provide_request_nonce)` on both the routes and the
        // fallback so every SSR render re-publishes the CSP nonce the
        // `security` middleware minted for this request — the inline
        // hydration `<script>` then matches `script-src 'nonce-…'`, which
        // carries no `'unsafe-inline'`.
        .leptos_routes_with_context(&leptos_options, routes, security::provide_request_nonce, {
            let leptos_options = leptos_options.clone();
            move || shell(leptos_options.clone())
        })
        .fallback(leptos_axum::file_and_error_handler_with_context(
            security::provide_request_nonce,
            shell,
        ))
        .with_state(leptos_options);

    let guarded_leptos_router = limits::apply_rate_limit(
        limits::apply(leptos_router),
        limits::SsrRateLimit::new(handshake),
    );

    guarded_leptos_router
        // Merged after the DoS guards so `/ws` keeps its long-lived
        // connection and `/api/rooms/:code` keeps its own rate limiter.
        .merge(signaling_router)
        .layer(axum::middleware::from_fn_with_state(
            dev_csp,
            security::apply,
        ))
}
