//! Response-header hardening. A middleware rather than a per-route
//! concern: every response — page, asset, JSON, the `/ws` upgrade — gets
//! the same set.

use axum::extract::{Request, State};
use axum::http::{header::HeaderName, HeaderValue};
use axum::middleware::Next;
use axum::response::Response;
use leptos::nonce::Nonce;

/// Request header the CSP middleware stamps with this response's nonce so
/// the Leptos render ([`provide_request_nonce`]) can put the *same* value
/// on the inline hydration `<script>` `leptos` emits — otherwise
/// `script-src` (which no longer carries `'unsafe-inline'`) would block
/// the framework's own bootstrap and the page would never hydrate.
pub const NONCE_REQUEST_HEADER: &str = "x-csp-nonce";

/// The `Content-Security-Policy` for one response.
///
/// `script-src` carries a per-request `'nonce-…'` instead of
/// `'unsafe-inline'`: the only inline script on the page is
/// `leptos`'s `HydrationScripts` / `AutoReload` bootstrap, which now
/// receives this nonce (see [`provide_request_nonce`]). Injected inline
/// script no longer runs.
///
/// `style-src` keeps `'unsafe-inline'`: Leptos binds dynamic values
/// through `style="--member: …"` *attributes*, and a CSP nonce only
/// covers `<style>` / `<script>` *elements*, never a `style=` attribute.
/// An inline style attribute can't execute script, so this is a far
/// smaller concession than `script-src 'unsafe-inline'` was.
///
/// `'wasm-unsafe-eval'` stays for the wasm-bindgen module. `connect-src`
/// gains plaintext `ws:` only outside production, for `cargo leptos
/// watch`'s live-reload socket on a second `ws://` port that the
/// production policy correctly blocks.
pub fn content_security_policy(nonce: &str, dev: bool) -> String {
    let dev_ws = if dev { " ws:" } else { "" };
    format!(
        "default-src 'self'; base-uri 'self'; object-src 'none'; \
         frame-ancestors 'none'; form-action 'self'; \
         img-src 'self' data:; font-src 'self' https://fonts.gstatic.com; \
         style-src 'self' 'unsafe-inline' https://fonts.googleapis.com; \
         script-src 'self' 'wasm-unsafe-eval' 'nonce-{nonce}'; \
         connect-src 'self' https: wss:{dev_ws}; media-src 'self' blob:; \
         worker-src 'self' blob:"
    )
}

/// Two years, per HSTS preload guidance. Safe: Fly terminates TLS and
/// `force_https` already redirects, so the app is only ever reached over
/// HTTPS in production.
pub const STRICT_TRANSPORT_SECURITY: &str = "max-age=63072000; includeSubDomains";

/// The web app captures a tab via `getDisplayMedia` (gated by
/// `display-capture`) and never uses geolocation. `camera` / `microphone`
/// are left at `self` rather than fully denied: Chrome cross-checks those
/// two feature policies when `getDisplayMedia({ audio: true })` runs (to
/// offer "share tab audio"), so `=()` there logs a console policy
/// violation on every share. `self` still blocks them in cross-origin
/// subframes, which is the point.
pub const PERMISSIONS_POLICY: &str =
    "camera=(self), microphone=(self), geolocation=(), display-capture=(self)";

/// The fixed (non-CSP) security headers this middleware sets, as
/// `(name, value)`. The `Content-Security-Policy` is built per request
/// (see [`content_security_policy`]) because it carries a fresh nonce.
pub fn static_headers() -> [(&'static str, &'static str); 6] {
    [
        ("strict-transport-security", STRICT_TRANSPORT_SECURITY),
        ("permissions-policy", PERMISSIONS_POLICY),
        ("x-content-type-options", "nosniff"),
        ("referrer-policy", "no-referrer"),
        ("x-frame-options", "DENY"),
        ("cross-origin-opener-policy", "same-origin"),
    ]
}

/// `axum::middleware::from_fn_with_state` handler: mint a CSP nonce, hand
/// it to the inner render via [`NONCE_REQUEST_HEADER`], run the inner
/// service, then stamp [`static_headers`] plus the nonce-bearing
/// `Content-Security-Policy` onto the response. The `bool` state is `true`
/// in a non-production (`cargo leptos watch`) run.
pub async fn apply(State(dev): State<bool>, mut request: Request, next: Next) -> Response {
    // 128 bits of CSPRNG, base64url — a valid CSP nonce token and a valid
    // header value.
    let nonce = Nonce::new().to_string();
    if let Ok(value) = HeaderValue::from_str(&nonce) {
        request
            .headers_mut()
            .insert(HeaderName::from_static(NONCE_REQUEST_HEADER), value);
    }

    let mut response = next.run(request).await;
    let response_headers = response.headers_mut();
    for (name, value) in static_headers() {
        response_headers.insert(
            HeaderName::from_static(name),
            HeaderValue::from_static(value),
        );
    }
    if let Ok(value) = HeaderValue::from_str(&content_security_policy(&nonce, dev)) {
        response_headers.insert(HeaderName::from_static("content-security-policy"), value);
    }
    response
}

/// Leptos context provider passed to `leptos_routes_with_context` /
/// `file_and_error_handler_with_context`: re-publishes the nonce
/// [`apply`] stamped on the request (`NONCE_REQUEST_HEADER`) as a
/// [`Nonce`], so `HydrationScripts` / `AutoReload` emit `nonce="…"`
/// matching the `Content-Security-Policy` header. Runs after
/// `leptos_axum` has put the request `Parts` into context (and after its
/// own `provide_nonce()`, which this overrides).
///
/// A no-op when the header is absent — e.g. a request that didn't pass
/// through [`apply`], such as a handler-level unit test. The framework
/// then keeps its own generated nonce, which simply won't match any
/// policy header.
pub fn provide_request_nonce() {
    use axum::http::request::Parts;
    use leptos::context::{provide_context, use_context};

    let Some(parts) = use_context::<Parts>() else {
        return;
    };
    let Some(nonce) = parts
        .headers
        .get(NONCE_REQUEST_HEADER)
        .and_then(|value| value.to_str().ok())
    else {
        return;
    };
    provide_context(Nonce::from_value(nonce.to_owned()));
}
