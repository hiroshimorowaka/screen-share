//! Unit tests for `turn` — kept in-crate (they exercise private
//! fields/consts) but split out of src/turn.rs to keep it readable (Phase 4).

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
    // Literal, not `CREDENTIAL_TTL.as_secs()`: asserting against the const
    // would move with any mutation of its own arithmetic and never catch
    // it. 1h = 3_600s; allow a few seconds of slack for the clock read.
    let ttl = expiry - now;
    assert!(
        (3_595..=3_600).contains(&ttl),
        "credential TTL should be 1 hour, got {ttl}s"
    );
}

const STRONG_SECRET: &str = "0123456789abcdef0123456789abcdef";

#[test]
fn from_vars_is_stun_only_when_no_secret_is_set() {
    let stun_only = |s, u| matches!(TurnConfig::from_vars(s, u), Ok(None));
    assert!(stun_only(None, None));
    assert!(stun_only(None, Some("turn:x:3478".into())));
    assert!(stun_only(Some(String::new()), Some("turn:x:3478".into())));
    assert!(stun_only(Some(String::new()), None));
}

#[test]
fn from_vars_errors_when_a_secret_is_set_without_urls() {
    // A secret is the deliberate "run TURN" signal; missing URLs then
    // means the relay is unreachable and clients would silently fall back
    // to STUN-only (P3 follow-up).
    assert!(matches!(
        TurnConfig::from_vars(Some(STRONG_SECRET.into()), None),
        Err(TurnConfigError::UrlsMissing)
    ));
    assert!(matches!(
        TurnConfig::from_vars(Some(STRONG_SECRET.into()), Some(String::new())),
        Err(TurnConfigError::UrlsMissing)
    ));
}

#[test]
fn from_vars_accepts_both_secret_and_urls() {
    assert!(matches!(
        TurnConfig::from_vars(Some(STRONG_SECRET.into()), Some("turn:x:3478".into())),
        Ok(Some(_))
    ));
}

#[test]
fn from_vars_rejects_a_short_or_placeholder_secret() {
    assert!(matches!(
        TurnConfig::from_vars(Some("short".into()), Some("turn:x:3478".into())),
        Err(TurnConfigError::SecretTooShort)
    ));
    assert!(matches!(
        TurnConfig::from_vars(Some("ChangeMe".into()), Some("turn:x:3478".into())),
        Err(TurnConfigError::SecretIsPlaceholder)
    ));
}

#[test]
fn from_vars_accepts_a_secret_exactly_at_the_minimum_length() {
    // `MIN_TURN_SECRET_LEN` chars is the shortest that passes; one fewer
    // is rejected (guards `<` vs `<=`).
    let at_min: String = "a".repeat(MIN_TURN_SECRET_LEN);
    let one_short: String = "a".repeat(MIN_TURN_SECRET_LEN - 1);

    assert!(matches!(
        TurnConfig::from_vars(Some(at_min), Some("turn:x:3478".into())),
        Ok(Some(_))
    ));
    assert!(matches!(
        TurnConfig::from_vars(Some(one_short), Some("turn:x:3478".into())),
        Err(TurnConfigError::SecretTooShort)
    ));
}

#[test]
fn turn_config_error_messages_are_non_empty_and_specific() {
    assert!(TurnConfigError::SecretTooShort
        .to_string()
        .contains("too short"));
    assert!(TurnConfigError::SecretIsPlaceholder
        .to_string()
        .contains("placeholder"));
    assert!(TurnConfigError::UrlsMissing
        .to_string()
        .contains("TURN_URLS"));
}

#[test]
fn from_vars_splits_and_trims_the_url_list() {
    let config = TurnConfig::from_vars(
        Some(STRONG_SECRET.into()),
        Some(" turn:a.example:3478 , turns:b.example:5349 ".into()),
    )
    .expect("valid config")
    .expect("both vars present and non-empty");

    assert_eq!(config.secret, STRONG_SECRET);
    assert_eq!(
        config.urls,
        vec![
            "turn:a.example:3478".to_string(),
            "turns:b.example:5349".to_string(),
        ]
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
