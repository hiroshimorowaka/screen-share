//! P3 follow-up: the HTTP (render / server-fn) routes had no body cap,
//! no per-request timeout, and no concurrency ceiling. `limits::apply`
//! adds all three; this checks the two that are deterministic to assert.

#![cfg(feature = "ssr")]

use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use axum::body::{to_bytes, Body, Bytes};
use axum::extract::{ConnectInfo, State};
use axum::http::{Request, StatusCode};
use axum::routing::{get, post};
use axum::Router;
use screen_share_server::middleware::limits::{
    apply, apply_rate_limit, SsrRateLimit, MAX_CONCURRENT_HTTP_REQUESTS, MAX_HTTP_BODY_BYTES,
    MAX_SSR_REQUESTS_PER_WINDOW,
};
use screen_share_signaling::handshake::HandshakeConfig;
use tokio::sync::watch;
use tower::ServiceExt;

// `Bytes` is a length-limited extractor, so `DefaultBodyLimit` applies —
// extracting the raw `Body` would bypass it.
async fn echo_len(body: Bytes) -> String {
    body.len().to_string()
}

fn body_of(n: usize) -> Body {
    Body::from(vec![b'x'; n])
}

#[tokio::test]
async fn a_body_over_the_cap_is_rejected_and_one_at_the_cap_is_accepted() {
    let app = apply(Router::new().route("/", post(echo_len)));

    let too_big = app
        .clone()
        .oneshot(
            Request::post("/")
                .body(body_of(MAX_HTTP_BODY_BYTES + 1))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(too_big.status(), StatusCode::PAYLOAD_TOO_LARGE);

    let at_cap = app
        .oneshot(
            Request::post("/")
                .body(body_of(MAX_HTTP_BODY_BYTES))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(at_cap.status(), StatusCode::OK);
    let seen = to_bytes(at_cap.into_body(), usize::MAX).await.unwrap();
    assert_eq!(seen, MAX_HTTP_BODY_BYTES.to_string().as_bytes());
}

fn rate_limited_app() -> Router {
    // `permissive()` => the client key is the TCP peer IP, which the test
    // sets per request via a `ConnectInfo` extension.
    apply_rate_limit(
        Router::new().route("/", get(|| async { "ok" })),
        SsrRateLimit::new(HandshakeConfig::permissive()),
    )
}

async fn get_from(app: &Router, peer: SocketAddr) -> StatusCode {
    app.clone()
        .oneshot(
            Request::builder()
                .uri("/")
                .extension(ConnectInfo(peer))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
        .status()
}

#[tokio::test]
async fn a_client_over_the_ssr_request_budget_gets_429_and_other_clients_are_unaffected() {
    let app = rate_limited_app();
    let flooder: SocketAddr = ([127, 0, 0, 1], 4001).into();
    let bystander: SocketAddr = ([127, 0, 0, 2], 4002).into();

    for _ in 0..MAX_SSR_REQUESTS_PER_WINDOW {
        assert_eq!(get_from(&app, flooder).await, StatusCode::OK);
    }
    assert_eq!(
        get_from(&app, flooder).await,
        StatusCode::TOO_MANY_REQUESTS,
        "the request past the per-client window budget is rejected"
    );
    assert_eq!(
        get_from(&app, bystander).await,
        StatusCode::OK,
        "a different client keeps its own budget"
    );
}

#[tokio::test]
async fn no_more_than_the_ceiling_of_requests_reach_a_handler_at_once() {
    let in_handler = Arc::new(AtomicUsize::new(0));
    // Flips to `true` once, letting every current and future handler
    // proceed — no missed-notification race.
    let (release_tx, release_rx) = watch::channel(false);

    let state = (in_handler.clone(), release_rx);
    let app = apply(
        Router::new()
            .route(
                "/",
                post(
                    |State((count, mut release)): State<(
                        Arc<AtomicUsize>,
                        watch::Receiver<bool>,
                    )>| async move {
                        count.fetch_add(1, Ordering::SeqCst);
                        while !*release.borrow_and_update() {
                            release.changed().await.unwrap();
                        }
                        StatusCode::OK
                    },
                ),
            )
            .with_state(state),
    );

    let extra = 5;
    let mut tasks = Vec::new();
    for _ in 0..(MAX_CONCURRENT_HTTP_REQUESTS + extra) {
        let app = app.clone();
        tasks.push(tokio::spawn(async move {
            app.oneshot(Request::post("/").body(Body::empty()).unwrap())
                .await
                .unwrap()
                .status()
        }));
    }

    // Give the spawned requests time to reach the handler (or be parked at
    // the concurrency layer).
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(
        in_handler.load(Ordering::SeqCst),
        MAX_CONCURRENT_HTTP_REQUESTS,
        "the concurrency layer must gate everything past the ceiling"
    );

    release_tx.send(true).unwrap();
    for task in tasks {
        assert_eq!(task.await.unwrap(), StatusCode::OK);
    }
    assert_eq!(
        in_handler.load(Ordering::SeqCst),
        MAX_CONCURRENT_HTTP_REQUESTS + extra,
        "the parked requests eventually get through"
    );
}
