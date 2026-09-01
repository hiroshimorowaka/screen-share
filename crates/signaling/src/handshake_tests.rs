//! Unit tests for `handshake` — kept in-crate (they build `HeaderMap`s
//! and check private behaviour) split out of src/handshake.rs like
//! `turn_tests.rs`.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use axum::http::{HeaderMap, HeaderName};

use super::{HandshakeConfig, OriginPolicy};

fn headers(pairs: &[(&str, &str)]) -> HeaderMap {
    let mut map = HeaderMap::new();
    for (name, value) in pairs {
        map.insert(
            HeaderName::from_bytes(name.as_bytes()).unwrap(),
            value.parse().unwrap(),
        );
    }
    map
}

fn peer(ip: [u8; 4]) -> SocketAddr {
    SocketAddr::new(IpAddr::V4(Ipv4Addr::from(ip)), 40000)
}

#[test]
fn parse_yields_allow_all_when_unset_or_empty() {
    assert_eq!(OriginPolicy::parse(None), OriginPolicy::AllowAll);
    assert_eq!(OriginPolicy::parse(Some("")), OriginPolicy::AllowAll);
    assert_eq!(OriginPolicy::parse(Some("  , ,")), OriginPolicy::AllowAll);
}

#[test]
fn parse_trims_and_collects_the_allowlist() {
    assert_eq!(
        OriginPolicy::parse(Some(" https://a.example , https://b.example ")),
        OriginPolicy::Allowlist(vec![
            "https://a.example".to_string(),
            "https://b.example".to_string(),
        ])
    );
}

#[test]
fn allow_all_permits_any_origin() {
    let policy = OriginPolicy::AllowAll;
    assert!(policy.permits(&headers(&[("origin", "https://evil.example")])));
}

#[test]
fn allowlist_rejects_an_unlisted_origin_but_accepts_a_listed_one() {
    let policy = OriginPolicy::parse(Some("https://app.example"));
    assert!(policy.permits(&headers(&[("origin", "https://app.example")])));
    assert!(!policy.permits(&headers(&[("origin", "https://evil.example")])));
}

#[test]
fn allowlist_accepts_a_request_with_no_origin_header() {
    // Native (non-browser) clients send no Origin; the check targets
    // browsers and a non-browser attacker gains nothing from spoofing it.
    let policy = OriginPolicy::parse(Some("https://app.example"));
    assert!(policy.permits(&HeaderMap::new()));
}

#[test]
fn client_key_uses_the_peer_ip_when_proxy_headers_are_not_trusted() {
    let config = HandshakeConfig::new(OriginPolicy::AllowAll, false);
    let key = config.client_key(
        &headers(&[("fly-client-ip", "203.0.113.9")]),
        peer([198, 51, 100, 7]),
    );
    assert_eq!(key, "198.51.100.7", "a spoofed header must be ignored");
}

#[test]
fn client_key_uses_the_forwarded_ip_only_when_proxy_headers_are_trusted() {
    let config = HandshakeConfig::new(OriginPolicy::AllowAll, true);
    assert_eq!(
        config.client_key(
            &headers(&[("fly-client-ip", "203.0.113.9")]),
            peer([198, 51, 100, 7])
        ),
        "203.0.113.9"
    );
}

#[test]
fn client_key_falls_back_to_the_peer_ip_when_the_trusted_header_is_absent() {
    let config = HandshakeConfig::new(OriginPolicy::AllowAll, true);
    assert_eq!(
        config.client_key(&HeaderMap::new(), peer([198, 51, 100, 7])),
        "198.51.100.7",
        "an absent header must not collapse clients onto one shared key"
    );
}
