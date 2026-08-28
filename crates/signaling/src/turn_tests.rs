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
