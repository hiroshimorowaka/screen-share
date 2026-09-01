//! Response-header hardening (finding F12). A middleware rather than a
//! per-route concern: every response — page, asset, JSON, the `/ws`
//! upgrade — gets the same set.

use axum::extract::Request;
use axum::http::{header::HeaderName, HeaderValue};
use axum::middleware::Next;
use axum::response::Response;

/// Directives are deliberately loose where the stack needs it:
/// `'wasm-unsafe-eval'` for the wasm-bindgen module, `'unsafe-inline'` in
/// `style-src` for Leptos's inline `style="--member: …"` bindings, and the
/// Google Fonts hosts (see `app.rs` `shell`). Everything else is `'self'`.
///
/// Must be smoke-tested in a real browser (create / join / watch, no CSP
/// violations in the console) — it is the one header here that can break
/// the UI.
pub const CONTENT_SECURITY_POLICY: &str = "default-src 'self'; base-uri 'self'; \
    object-src 'none'; frame-ancestors 'none'; form-action 'self'; \
    img-src 'self' data:; font-src 'self' https://fonts.gstatic.com; \
    style-src 'self' 'unsafe-inline' https://fonts.googleapis.com; \
    script-src 'self' 'wasm-unsafe-eval'; connect-src 'self' https: wss:; \
    media-src 'self' blob:; worker-src 'self' blob:";

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
