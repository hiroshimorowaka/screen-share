//! `screen-share-server` — the Axum host. Renders Leptos pages
//! server-side and runs the signaling relay. All meaning of the signaling
//! messages lives in the browser (`screen_share`'s `hydrate` build); this
//! binary only routes them.
#![recursion_limit = "512"]
#![warn(clippy::too_many_lines)]
#![warn(clippy::too_many_arguments)]

#[cfg(feature = "ssr")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    use leptos::logging::log;
    use screen_share_server::config::ServerConfig;
    use screen_share_server::router;
    use screen_share_signaling::registry::Registry;
    use screen_share_signaling::rooms_status::RoomStatusLimiter;
    use screen_share_signaling::state::SignalingState;

    let cfg = ServerConfig::from_env()?;
    let addr = cfg.leptos_options.site_addr;

    log!(
        "TURN server: {}",
        if cfg.turn.is_some() {
            "configured"
        } else {
            "not configured (STUN-only ICE)"
        }
    );

    let signaling_state = SignalingState {
        registry: Registry::new(),
        turn: cfg.turn,
        handshake: cfg.handshake.clone(),
        room_status_limiter: RoomStatusLimiter::new(),
    };

    let app = router::build(
        cfg.leptos_options.clone(),
        signaling_state,
        cfg.handshake,
        cfg.dev_csp,
    );

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
fn main() {}
