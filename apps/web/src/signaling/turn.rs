use std::time::{Duration, SystemTime, UNIX_EPOCH};

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine as _;
use hmac::{Hmac, Mac};
use sha1::Sha1;

use screen_share_protocol::TurnCredentials;

/// How long a minted credential stays valid. Only gates *new* TURN
/// allocations — coturn checks a credential's embedded expiry only at
/// allocation time, so this doesn't cut off media already flowing through
/// an allocation made before it lapsed. Long enough to cover a room session
/// without the client re-requesting one, short enough that a leaked
/// credential stops being useful on its own.
const CREDENTIAL_TTL: Duration = Duration::from_secs(6 * 60 * 60);

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
        let secret = std::env::var("TURN_SECRET")
            .ok()
            .filter(|s| !s.is_empty())?;
        let urls_raw = std::env::var("TURN_URLS").ok().filter(|s| !s.is_empty())?;
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
mod tests {
    use super::*;

    fn config() -> TurnConfig {
        TurnConfig {
            secret: "s3cr3t".to_string(),
            urls: vec!["turn:example.com:3478".to_string()],
        }
    }

    #[test]
    fn mint_credentials_carries_the_configured_urls() {
        let creds = config().mint_credentials();
        assert_eq!(creds.urls, vec!["turn:example.com:3478".to_string()]);
    }

    #[test]
    fn mint_credentials_username_is_a_future_unix_timestamp() {
        let creds = config().mint_credentials();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let expiry: u64 = creds
            .username
            .parse()
            .expect("username should be a Unix timestamp");
        assert!(expiry > now, "expiry should be in the future");
        assert!(
            expiry <= now + CREDENTIAL_TTL.as_secs() + 1,
            "expiry shouldn't exceed the configured TTL"
        );
    }

    #[test]
    fn mint_credentials_password_is_the_hmac_sha1_of_the_username() {
        let creds = config().mint_credentials();

        let mut mac = Hmac::<Sha1>::new_from_slice(b"s3cr3t").unwrap();
        mac.update(creds.username.as_bytes());
        let expected = BASE64.encode(mac.finalize().into_bytes());

        assert_eq!(creds.password, expected);
    }
}
