//! The security-headers middleware (finding F12) is applied to every
//! response.

#![cfg(feature = "ssr")]

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::routing::get;
use axum::Router;
use screen_share::http_security;
use tower::ServiceExt;

fn app() -> Router {
    Router::new()
        .route("/", get(|| async { "ok" }))
        .layer(axum::middleware::from_fn(http_security::apply))
}

#[tokio::test]
async fn every_declared_security_header_is_on_the_response() {
    let response = app()
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    for (name, expected) in http_security::HEADERS {
        let got = response
            .headers()
            .get(*name)
            .unwrap_or_else(|| panic!("missing header {name}"))
            .to_str()
            .unwrap();
        assert_eq!(got, *expected, "header {name}");
    }
}

#[tokio::test]
async fn the_csp_allows_the_stack_the_app_actually_needs() {
    // A regression guard on the deliberately-loose directives — dropping
    // one of these silently breaks hydration or the fonts in production.
    let csp = http_security::CONTENT_SECURITY_POLICY;
    for needed in [
        "default-src 'self'",
        "script-src 'self' 'wasm-unsafe-eval'",
        "style-src 'self' 'unsafe-inline' https://fonts.googleapis.com",
        "font-src 'self' https://fonts.gstatic.com",
        "connect-src 'self' https: wss:",
        "frame-ancestors 'none'",
    ] {
        assert!(csp.contains(needed), "CSP missing `{needed}`");
    }
}
