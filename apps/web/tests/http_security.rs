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

    for (name, expected) in http_security::static_headers() {
        let got = response
            .headers()
            .get(name)
            .unwrap_or_else(|| panic!("missing header {name}"))
            .to_str()
            .unwrap();
        assert_eq!(got, expected, "header {name}");
    }
    assert!(
        response.headers().contains_key("content-security-policy"),
        "the CSP header is set per request, not part of static_headers()"
    );
}

#[tokio::test]
async fn the_prod_csp_allows_the_stack_the_app_needs_but_not_plaintext_ws() {
    let csp = header_value(false, "content-security-policy").await;
    for needed in [
        "default-src 'self'",
        // Inline hydration bootstrap is nonce-allowed, not blanket-inline.
        "script-src 'self' 'wasm-unsafe-eval' 'nonce-",
        "style-src 'self' 'unsafe-inline' https://fonts.googleapis.com",
        "font-src 'self' https://fonts.gstatic.com",
        "connect-src 'self' https: wss:",
        "frame-ancestors 'none'",
    ] {
        assert!(csp.contains(needed), "prod CSP missing `{needed}`: {csp}");
    }
    assert!(
        !csp.contains("script-src 'self' 'wasm-unsafe-eval' 'unsafe-inline'"),
        "script-src must not fall back to 'unsafe-inline' (finding A-02): {csp}"
    );
    assert!(
        !csp.contains(" ws:"),
        "prod CSP must not allow plaintext ws:"
    );
}

#[tokio::test]
async fn each_response_gets_a_fresh_csp_nonce() {
    let first = header_value(false, "content-security-policy").await;
    let second = header_value(false, "content-security-policy").await;

    let a = csp_nonce(&first);
    let b = csp_nonce(&second);
    assert!(!a.is_empty() && !b.is_empty(), "nonce is non-empty");
    assert_ne!(a, b, "a new nonce is minted per response");
}

/// The `'nonce-…'` token out of a `script-src` directive.
fn csp_nonce(csp: &str) -> String {
    let start = csp.find("'nonce-").expect("script-src carries a nonce") + "'nonce-".len();
    let rest = &csp[start..];
    rest[..rest.find('\'').expect("nonce is quoted")].to_owned()
}

/// End-to-end: a real Leptos SSR render behind the middleware must put the
/// *same* nonce on `HydrationScripts`' inline `<script>` as the one in the
/// `Content-Security-Policy` header — otherwise the page can't hydrate
/// (finding A-02: `script-src` no longer allows `'unsafe-inline'`).
#[tokio::test]
async fn the_hydration_bootstrap_script_carries_the_csp_nonce() {
    use leptos::config::LeptosOptions;
    use leptos::prelude::*;
    use leptos_axum::LeptosRoutes;
    use leptos_meta::MetaTags;
    use leptos_router::components::{Route, Router, Routes};
    use leptos_router::StaticSegment;

    #[component]
    fn TestApp() -> impl IntoView {
        view! {
            <Router>
                <Routes fallback=|| view! { "nope" }>
                    <Route path=StaticSegment("") view=|| view! { <p>"ok"</p> } />
                </Routes>
            </Router>
        }
    }

    fn test_shell(options: LeptosOptions) -> impl IntoView {
        view! {
            <!DOCTYPE html>
            <html>
                <head>
                    <HydrationScripts options/>
                    <MetaTags/>
                </head>
                <body>
                    <TestApp/>
                </body>
            </html>
        }
    }

    let options = LeptosOptions::builder().output_name("test").build();
    let routes = leptos_axum::generate_route_list(TestApp);
    let router = Router::<LeptosOptions>::new()
        .leptos_routes_with_context(&options, routes, http_security::provide_request_nonce, {
            let options = options.clone();
            move || test_shell(options.clone())
        })
        .with_state(options)
        .layer(axum::middleware::from_fn_with_state(
            false,
            http_security::apply,
        ));

    let response = router
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let csp = response
        .headers()
        .get("content-security-policy")
        .unwrap()
        .to_str()
        .unwrap()
        .to_owned();
    let nonce = csp_nonce(&csp);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let html = String::from_utf8_lossy(&body);

    assert!(
        html.contains(&format!("nonce=\"{nonce}\"")),
        "hydration <script> must carry nonce {nonce}; html was: {html}"
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
