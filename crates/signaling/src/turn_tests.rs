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
    // it. 6h = 21_600s; allow a second of slack for the clock read.
    let ttl = expiry - now;
    assert!(
        (21_595..=21_600).contains(&ttl),
        "credential TTL should be 6 hours, got {ttl}s"
    );
}

#[test]
fn from_vars_needs_both_secret_and_urls_non_empty() {
    assert!(TurnConfig::from_vars(None, Some("turn:x:3478".into())).is_none());
    assert!(TurnConfig::from_vars(Some("s".into()), None).is_none());
    assert!(TurnConfig::from_vars(Some(String::new()), Some("turn:x:3478".into())).is_none());
    assert!(TurnConfig::from_vars(Some("s".into()), Some(String::new())).is_none());
    assert!(TurnConfig::from_vars(Some("s".into()), Some("turn:x:3478".into())).is_some());
}

#[test]
fn from_vars_splits_and_trims_the_url_list() {
    let config = TurnConfig::from_vars(
        Some("s3cr3t".into()),
        Some(" turn:a.example:3478 , turns:b.example:5349 ".into()),
    )
    .expect("both vars present and non-empty");

    assert_eq!(config.secret, "s3cr3t");
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
