//! Response-header hardening (finding F12). A middleware rather than a
//! per-route concern: every response — page, asset, JSON, the `/ws`
//! upgrade — gets the same set.

use axum::extract::Request;
use axum::http::{header::HeaderName, HeaderValue};
use axum::middleware::Next;
use axum::response::Response;

/// Directives are deliberately loose where the stack needs it:
///
/// - `script-src` includes `'wasm-unsafe-eval'` (the wasm-bindgen module)
///   and `'unsafe-inline'` — `leptos_meta::HydrationScripts` emits an
///   inline `<script type="module">` bootstrap with no nonce (the `leptos`
///   `nonce` feature isn't enabled), which a nonce-less / hash-less
///   `script-src` would block, leaving the page un-hydrated. Tightening
///   this to a nonce is a follow-up (enable the feature, inject the nonce
///   into this header per request).
/// - `style-src` includes `'unsafe-inline'` for Leptos's
///   `style="--member: …"` bindings, plus the Google Fonts stylesheet
///   host (see `app.rs` `shell`).
///
/// Everything else is `'self'`. Still worth smoke-testing in a real
/// browser (create / join / watch, no CSP violations logged).
pub const CONTENT_SECURITY_POLICY: &str = "default-src 'self'; base-uri 'self'; \
    object-src 'none'; frame-ancestors 'none'; form-action 'self'; \
    img-src 'self' data:; font-src 'self' https://fonts.gstatic.com; \
    style-src 'self' 'unsafe-inline' https://fonts.googleapis.com; \
    script-src 'self' 'wasm-unsafe-eval' 'unsafe-inline'; \
    connect-src 'self' https: wss:; media-src 'self' blob:; worker-src 'self' blob:";

/// Two years, per HSTS preload guidance. Safe: Fly terminates TLS and
/// `force_https` already redirects, so the app is only ever reached over
/// HTTPS in production.
pub const STRICT_TRANSPORT_SECURITY: &str = "max-age=63072000; includeSubDomains";

/// The web app captures a tab via `getDisplayMedia` (gated by
/// `display-capture`); it never uses the camera, mic, or geolocation.
pub const PERMISSIONS_POLICY: &str =
    "camera=(), microphone=(), geolocation=(), display-capture=(self)";

/// Every header this middleware sets, as `(name, value)` — also the fixture
/// the test asserts against.
pub const HEADERS: &[(&str, &str)] = &[
    ("content-security-policy", CONTENT_SECURITY_POLICY),
    ("strict-transport-security", STRICT_TRANSPORT_SECURITY),
    ("permissions-policy", PERMISSIONS_POLICY),
    ("x-content-type-options", "nosniff"),
    ("referrer-policy", "no-referrer"),
    ("x-frame-options", "DENY"),
    ("cross-origin-opener-policy", "same-origin"),
];

/// `axum::middleware::from_fn` handler: run the inner service, then stamp
/// [`HEADERS`] onto the response.
pub async fn apply(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    for (name, value) in HEADERS {
        headers.insert(
            HeaderName::from_static(name),
            HeaderValue::from_static(value),
        );
    }
    response
}
