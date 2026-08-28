//! Unit tests for `ws` — kept in-crate (they exercise the private
//! `client_key` helper) but split out of src/ws.rs to keep it readable,
//! matching the pattern used by `turn_tests.rs`.

use axum::http::HeaderMap;

use super::client_key;

#[test]
fn client_key_uses_the_fly_client_ip_header_when_present() {
    let mut headers = HeaderMap::new();
    headers.insert("fly-client-ip", "203.0.113.7".parse().unwrap());

    assert_eq!(client_key(&headers), "203.0.113.7");
}

#[test]
fn client_key_falls_back_to_a_constant_without_the_header() {
    assert_eq!(client_key(&HeaderMap::new()), "unknown");
}

#[test]
fn client_key_falls_back_when_the_header_is_not_valid_utf8() {
    let mut headers = HeaderMap::new();
    headers.insert(
        "fly-client-ip",
        axum::http::HeaderValue::from_bytes(&[0xff, 0xfe]).unwrap(),
    );

    assert_eq!(client_key(&headers), "unknown");
}
