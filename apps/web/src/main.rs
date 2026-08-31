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
    use screen_share_signaling::registry::Registry;
    use screen_share_signaling::rooms_status::room_status_handler;
    use screen_share_signaling::state::SignalingState;
    use screen_share_signaling::turn::TurnConfig;
    use screen_share_signaling::ws::ws_handler;

    let conf = get_configuration(None)?;
    let addr = conf.leptos_options.site_addr;
    let leptos_options = conf.leptos_options;
    let routes = generate_route_list(App);

    let turn = TurnConfig::from_env();
    log!(
        "TURN server: {}",
        if turn.is_some() {
            "configured"
        } else {
            "not configured (STUN-only ICE)"
        }
    );
    let signaling_state = SignalingState {
        registry: Registry::new(),
        turn,
    };
    let signaling_router = Router::new()
        .route("/ws", get(ws_handler))
        .route("/api/rooms/{code}", get(room_status_handler))
        .with_state(signaling_state);

    let app = Router::new()
        .leptos_routes(&leptos_options, routes, {
            let leptos_options = leptos_options.clone();
            move || shell(leptos_options.clone())
        })
        .fallback(leptos_axum::file_and_error_handler(shell))
        .with_state(leptos_options)
        .merge(signaling_router);

    log!("listening on http://{}", &addr);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app.into_make_service()).await?;
    Ok(())
}

#[cfg(not(feature = "ssr"))]
pub fn main() {}
