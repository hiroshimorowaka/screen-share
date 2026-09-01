//! The security-headers middleware (finding F12) is applied to every
//! response.

#![cfg(feature = "ssr")]

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::routing::get;
use axum::Router;
use screen_share::http_security;
use tower::ServiceExt;

fn app(dev: bool) -> Router {
    Router::new()
        .route("/", get(|| async { "ok" }))
        .layer(axum::middleware::from_fn_with_state(
            dev,
            http_security::apply,
        ))
}

async fn header_value(dev: bool, name: &str) -> String {
    let response = app(dev)
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    response
        .headers()
        .get(name)
        .unwrap_or_else(|| panic!("missing header {name}"))
        .to_str()
        .unwrap()
        .to_owned()
}

#[tokio::test]
async fn every_declared_security_header_is_on_the_response() {
    let response = app(false)
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();

    for (name, expected) in http_security::headers(false) {
        let got = response
            .headers()
            .get(name)
            .unwrap_or_else(|| panic!("missing header {name}"))
            .to_str()
            .unwrap();
        assert_eq!(got, expected, "header {name}");
    }
}

#[tokio::test]
async fn the_prod_csp_allows_the_stack_the_app_needs_but_not_plaintext_ws() {
    let csp = header_value(false, "content-security-policy").await;
    for needed in [
        "default-src 'self'",
        "script-src 'self' 'wasm-unsafe-eval' 'unsafe-inline'",
        "style-src 'self' 'unsafe-inline' https://fonts.googleapis.com",
        "font-src 'self' https://fonts.gstatic.com",
        "connect-src 'self' https: wss:",
        "frame-ancestors 'none'",
    ] {
        assert!(csp.contains(needed), "prod CSP missing `{needed}`");
    }
    assert!(
        !csp.contains(" ws:"),
        "prod CSP must not allow plaintext ws:"
    );
}

#[tokio::test]
async fn the_dev_csp_additionally_allows_the_cargo_leptos_reload_socket() {
    let csp = header_value(true, "content-security-policy").await;
    assert!(
        csp.contains("connect-src 'self' https: wss: ws:"),
        "dev CSP must allow ws: for the live-reload socket, got: {csp}"
    );
}
