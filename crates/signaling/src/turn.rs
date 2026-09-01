use std::time::{Duration, SystemTime, UNIX_EPOCH};

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine as _;
use hmac::{Hmac, Mac};
use sha1::Sha1;

use screen_share_protocol::TurnCredentials;

/// How long a minted credential stays valid. Only gates *new* TURN
/// allocations — coturn checks a credential's embedded expiry only at
/// allocation time, so this doesn't cut off media already flowing through
/// an allocation made before it lapsed. One hour: comfortably longer than
/// a room session (a fresh credential is minted on every `Joined`, i.e.
/// every reconnect), short enough that a credential leaked from a `Joined`
/// snapshot stops being a usable relay handle quickly. Was 6h; shortened
/// as part of the coturn hardening (see docker-entrypoint.sh) so a leaked
/// credential has a small window even before the peer-IP allowlist and
/// quotas blunt what it can do.
const CREDENTIAL_TTL: Duration = Duration::from_secs(60 * 60);

/// A deployment's TURN setup, read once from the environment at startup.
/// `None` (no `TurnConfig` at all) means this deployment has no TURN
/// server configured — callers hand clients STUN-only ICE in that case.
#[derive(Clone)]
pub struct TurnConfig {
    /// Must match coturn's `static-auth-secret` exactly.
    secret: String,
    urls: Vec<String>,
}

impl TurnConfig {
    /// `TURN_SECRET` and `TURN_URLS` (comma-separated `turn:`/`turns:` URLs)
    /// must both be set and non-empty, or this deployment runs STUN-only.
    pub fn from_env() -> Option<Self> {
        Self::from_vars(
            std::env::var("TURN_SECRET").ok(),
            std::env::var("TURN_URLS").ok(),
        )
    }

    /// The parsing/validation half of [`from_env`](Self::from_env), split
    /// out so it can be built directly in tests without mutating
    /// process-global env vars. Both values must be present and non-empty;
    /// `urls` is split on commas and each entry trimmed.
    pub fn from_vars(secret: Option<String>, urls_raw: Option<String>) -> Option<Self> {
        let secret = secret.filter(|s| !s.is_empty())?;
        let urls_raw = urls_raw.filter(|s| !s.is_empty())?;
        let urls: Vec<String> = urls_raw
            .split(',')
            .map(|url| url.trim().to_string())
            .collect();
        Some(Self { secret, urls })
    }

    /// A time-limited credential per the REST API scheme coturn implements
    /// via `use-auth-secret` (the de facto standard for TURN auth without
    /// per-user accounts): the username is a Unix timestamp the credential
    /// expires at, and the password is an HMAC-SHA1 of that username under
    /// the shared secret — so coturn can verify a credential itself, with
    /// no shared state beyond the secret.
    pub fn mint_credentials(&self) -> TurnCredentials {
        let expires_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should read a time after the Unix epoch")
            + CREDENTIAL_TTL;
        let username = expires_at.as_secs().to_string();

        let mut mac = Hmac::<Sha1>::new_from_slice(self.secret.as_bytes())
            .expect("HMAC-SHA1 accepts a key of any length");
        mac.update(username.as_bytes());
        let password = BASE64.encode(mac.finalize().into_bytes());

        TurnCredentials {
            urls: self.urls.clone(),
            username,
            password,
        }
    }
}

#[cfg(test)]
#[path = "turn_tests.rs"]
mod tests;
