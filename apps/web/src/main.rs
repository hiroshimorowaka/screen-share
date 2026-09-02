// The bin monomorphises the lib's `view!` types when it renders routes;
// `RoomPage`'s is deep enough to need this above the default (see the same
// attribute on `lib.rs`).
#![recursion_limit = "512"]

#[cfg(feature = "ssr")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    use axum::routing::get;
    use axum::Router;
    use leptos::logging::log;
    use leptos::prelude::*;
    use leptos_axum::{generate_route_list, LeptosRoutes};
    use screen_share::app::{shell, App};
    use screen_share::{http_limits, http_security};
    use screen_share_signaling::handshake::HandshakeConfig;
    use screen_share_signaling::registry::Registry;
    use screen_share_signaling::rooms_status::{room_status_handler, RoomStatusLimiter};
    use screen_share_signaling::state::SignalingState;
    use screen_share_signaling::turn::TurnConfig;
    use screen_share_signaling::ws::ws_handler;

    let conf = get_configuration(None)?;
    let addr = conf.leptos_options.site_addr;
    let leptos_options = conf.leptos_options;
    // Non-PROD => `cargo leptos watch`: its live-reload WebSocket needs a
    // slightly looser CSP (see `http_security`).
    let dev_csp = !matches!(leptos_options.env, leptos::config::Env::PROD);
    let routes = generate_route_list(App);

    // Abort startup on a misconfigured TURN secret rather than silently
    // running STUN-only or, worse, a relay with a weak secret (F13).
    let turn = TurnConfig::from_env()?;
    log!(
        "TURN server: {}",
        if turn.is_some() {
            "configured"
        } else {
            "not configured (STUN-only ICE)"
        }
    );
    let handshake = HandshakeConfig::from_env();
    let signaling_state = SignalingState {
        registry: Registry::new(),
        turn,
        handshake: handshake.clone(),
        room_status_limiter: RoomStatusLimiter::new(),
    };
    let signaling_router = Router::new()
        .route("/ws", get(ws_handler))
        .route("/api/rooms/{code}", get(room_status_handler))
        .with_state(signaling_state);

    let leptos_router = Router::new()
        // `_with_context(provide_request_nonce)` on both the routes and the
        // fallback so every SSR render re-publishes the CSP nonce the
        // `http_security` middleware minted for this request — the inline
        // hydration `<script>` then matches `script-src 'nonce-…'` (F12
        // follow-up: `script-src` no longer carries `'unsafe-inline'`).
        .leptos_routes_with_context(
            &leptos_options,
            routes,
            http_security::provide_request_nonce,
            {
                let leptos_options = leptos_options.clone();
                move || shell(leptos_options.clone())
            },
        )
        .fallback(leptos_axum::file_and_error_handler_with_context(
            http_security::provide_request_nonce,
            shell,
        ))
        .with_state(leptos_options);

    let guarded_leptos_router = http_limits::apply_rate_limit(
        http_limits::apply(leptos_router),
        http_limits::SsrRateLimit::new(handshake),
    );
    let app = guarded_leptos_router
        // Merged after the DoS guards so `/ws` keeps its long-lived
        // connection and `/api/rooms/:code` keeps its own rate limiter.
        .merge(signaling_router)
        .layer(axum::middleware::from_fn_with_state(
            dev_csp,
            http_security::apply,
        ));

    log!("listening on http://{}", &addr);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    // `with_connect_info` so the signaling handler can read the real TCP
    // peer address (see `HandshakeConfig::client_key`).
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .await?;
    Ok(())
}

#[cfg(not(feature = "ssr"))]
pub fn main() {}
