use std::fmt;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine as _;
use hmac::{Hmac, Mac};
use sha1::Sha1;

use screen_share_protocol::TurnCredentials;

/// Shortest `TURN_SECRET` accepted. coturn's `use-auth-secret` scheme is
/// only as strong as this secret — a short one is brute-forceable, so a
/// misconfiguration should stop the process at boot rather than run a
/// relay anyone can mint credentials for (finding F13). 32 hex chars is
/// what the deploy docs already recommend (`openssl rand -hex 32` = 64).
const MIN_TURN_SECRET_LEN: usize = 24;

/// Obvious placeholder secrets, rejected regardless of length so a
/// copy-pasted example can't reach production.
const TURN_SECRET_DENYLIST: &[&str] = &[
    "changeme",
    "change-me",
    "secret",
    "turnsecret",
    "turn-secret",
    "password",
    "screenshare",
    "example",
    "test",
    "placeholder",
];

/// Why [`TurnConfig::from_vars`] refused a configured TURN setup. Returned
/// (not swallowed as "STUN-only") so a deploy that *meant* to run TURN
/// fails loudly instead of silently degrading.
#[derive(Debug, PartialEq, Eq)]
pub enum TurnConfigError {
    /// `TURN_SECRET` is shorter than [`MIN_TURN_SECRET_LEN`].
    SecretTooShort,
    /// `TURN_SECRET` is a known placeholder (see [`TURN_SECRET_DENYLIST`]).
    SecretIsPlaceholder,
    /// `TURN_SECRET` is set but `TURN_URLS` is empty/unset. Without URLs
    /// the relay is unreachable, so clients silently fell back to
    /// STUN-only despite a secret being configured — a misconfiguration
    /// that should abort the boot, not run half a TURN setup.
    UrlsMissing,
}

impl fmt::Display for TurnConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SecretTooShort => write!(
                f,
                "TURN_SECRET is too short (need at least {MIN_TURN_SECRET_LEN} characters; \
                 use `openssl rand -hex 32`)"
            ),
            Self::SecretIsPlaceholder => {
                write!(f, "TURN_SECRET is a well-known placeholder value")
            }
            Self::UrlsMissing => write!(
                f,
                "TURN_SECRET is set but TURN_URLS is empty (set the comma-separated \
                 turn:/turns: URL list, or unset TURN_SECRET for STUN-only)"
            ),
        }
    }
}

impl std::error::Error for TurnConfigError {}

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
    ///
    /// # Errors
    ///
    /// [`TurnConfigError`] when TURN *is* configured but `TURN_SECRET`
    /// fails validation — the process should abort rather than run a weak
    /// relay.
    pub fn from_env() -> Result<Option<Self>, TurnConfigError> {
        Self::from_vars(
            std::env::var("TURN_SECRET").ok(),
            std::env::var("TURN_URLS").ok(),
        )
    }

    /// The parsing/validation half of [`from_env`](Self::from_env), split
    /// out so it can be built directly in tests without mutating
    /// process-global env vars.
    ///
    /// No secret (absent/empty) ⇒ `Ok(None)` (STUN-only, a valid choice),
    /// whatever `TURN_URLS` says. Secret present but no URLs ⇒
    /// [`TurnConfigError::UrlsMissing`]. Both present ⇒ the secret is
    /// validated ([`MIN_TURN_SECRET_LEN`], [`TURN_SECRET_DENYLIST`]) and
    /// `urls` is split on commas and trimmed.
    ///
    /// # Errors
    ///
    /// [`TurnConfigError`] if TURN is half-configured (secret without
    /// URLs) or the configured secret is too short or a known placeholder.
    pub fn from_vars(
        secret: Option<String>,
        urls_raw: Option<String>,
    ) -> Result<Option<Self>, TurnConfigError> {
        let Some(secret) = secret.filter(|s| !s.is_empty()) else {
            return Ok(None);
        };
        let Some(urls_raw) = urls_raw.filter(|s| !s.is_empty()) else {
            return Err(TurnConfigError::UrlsMissing);
        };

        if TURN_SECRET_DENYLIST
            .iter()
            .any(|weak| weak.eq_ignore_ascii_case(&secret))
        {
            return Err(TurnConfigError::SecretIsPlaceholder);
        }
        if secret.chars().count() < MIN_TURN_SECRET_LEN {
            return Err(TurnConfigError::SecretTooShort);
        }

        let urls: Vec<String> = urls_raw
            .split(',')
            .map(|url| url.trim().to_string())
            .collect();
        Ok(Some(Self { secret, urls }))
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
