//! Policy applied to a `/ws` upgrade request before it becomes a
//! signaling connection: which `Origin`s may open one, and whether the
//! `fly-client-ip` header can be trusted for per-client rate limiting.
//!
//! Both are read once from the environment at startup. Neither is
//! authentication — a non-browser client sets any header it likes — they
//! are defence in depth (OWASP's WebSocket guidance) and a correctness
//! fix for the rate-limit key.

use std::net::SocketAddr;

use axum::http::HeaderMap;

/// Which `Origin` headers are allowed to open a signaling socket.
#[derive(Clone, Debug, PartialEq)]
pub enum OriginPolicy {
    /// No `SIGNALING_ALLOWED_ORIGINS` configured — every origin is
    /// accepted. The default so local dev and any deployment that hasn't
    /// set the variable keep working; production sets the variable.
    AllowAll,
    /// Only these exact origins (scheme + host + port), plus requests
    /// that carry no `Origin` header at all (native/non-browser
    /// clients, which gain nothing from the check).
    Allowlist(Vec<String>),
}

impl OriginPolicy {
    /// Reads `SIGNALING_ALLOWED_ORIGINS` — a comma-separated list of
    /// origins. Empty or unset yields [`OriginPolicy::AllowAll`].
    pub fn from_env() -> Self {
        Self::parse(std::env::var("SIGNALING_ALLOWED_ORIGINS").ok().as_deref())
    }

    /// The parsing half of [`from_env`](Self::from_env), split out so it
    /// can be exercised without touching process-global env vars.
    pub fn parse(raw: Option<&str>) -> Self {
        let origins: Vec<String> = raw
            .unwrap_or("")
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .collect();
        if origins.is_empty() {
            Self::AllowAll
        } else {
            Self::Allowlist(origins)
        }
    }

    /// Whether a handshake carrying `headers` may proceed.
    pub fn permits(&self, headers: &HeaderMap) -> bool {
        match self {
            Self::AllowAll => true,
            Self::Allowlist(allowed) => match headers.get("origin") {
                None => true,
                Some(value) => value
                    .to_str()
                    .is_ok_and(|origin| allowed.iter().any(|a| a == origin)),
            },
        }
    }
}

/// Everything the `/ws` handler needs to decide about a handshake,
/// resolved once at startup.
#[derive(Clone, Debug, PartialEq)]
pub struct HandshakeConfig {
    origin_policy: OriginPolicy,
    /// When `true`, `fly-client-ip` is used as the rate-limit key; only
    /// safe behind a proxy (Fly's edge) that overwrites it. When `false`,
    /// the real TCP peer address is used instead, so a client can't
    /// rotate a spoofed header to escape the wrong-password lockout, and
    /// an absent header can't collapse every client onto one shared key.
    trust_proxy_headers: bool,
}

impl HandshakeConfig {
    /// `SIGNALING_ALLOWED_ORIGINS` (see [`OriginPolicy`]) and
    /// `TRUST_PROXY_HEADERS` (`1`/`true`/`yes`, case-insensitive).
    pub fn from_env() -> Self {
        Self {
            origin_policy: OriginPolicy::from_env(),
            trust_proxy_headers: matches!(
                std::env::var("TRUST_PROXY_HEADERS")
                    .unwrap_or_default()
                    .to_ascii_lowercase()
                    .as_str(),
                "1" | "true" | "yes"
            ),
        }
    }

    /// Accept any origin, don't trust proxy headers — the shape a plain
    /// local run or a test gets.
    pub fn permissive() -> Self {
        Self {
            origin_policy: OriginPolicy::AllowAll,
            trust_proxy_headers: false,
        }
    }

    /// Constructor with both knobs set explicitly (used by tests).
    pub fn new(origin_policy: OriginPolicy, trust_proxy_headers: bool) -> Self {
        Self {
            origin_policy,
            trust_proxy_headers,
        }
    }

    pub fn permits_origin(&self, headers: &HeaderMap) -> bool {
        self.origin_policy.permits(headers)
    }

    /// The key the wrong-password lockout and per-client room cap are
    /// scoped to: the forwarded client IP when proxy headers are trusted
    /// and present, otherwise the real TCP peer address. Never a shared
    /// constant.
    pub fn client_key(&self, headers: &HeaderMap, peer: SocketAddr) -> String {
        if self.trust_proxy_headers {
            if let Some(ip) = headers.get("fly-client-ip").and_then(|v| v.to_str().ok()) {
                if !ip.is_empty() {
                    return ip.to_owned();
                }
            }
        }
        peer.ip().to_string()
    }
}

#[cfg(test)]
#[path = "handshake_tests.rs"]
mod tests;
