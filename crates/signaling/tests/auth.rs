//! Moved out of src/auth.rs (refactor Phase 4).

use screen_share_signaling::auth::*;

#[test]
fn hash_password_does_not_store_plaintext() {
    let hash = hash_password("minha-senha-123");
    assert_ne!(hash, "minha-senha-123");
    assert!(!hash.is_empty());
}

#[test]
fn verify_password_accepts_correct_password() {
    let hash = hash_password("minha-senha-123");
    assert!(verify_password("minha-senha-123", &hash));
}

#[test]
fn verify_password_rejects_wrong_password() {
    let hash = hash_password("minha-senha-123");
    assert!(!verify_password("senha-errada", &hash));
}

/// Finding F02: the argon2 cost was lowered so a burst of unauthenticated
/// `verify_password` calls can't OOM the 256 MB VM. Argon2 embeds its
/// parameters in the PHC string, so a hash written with the *old*
/// `Argon2::default()` cost (19 MiB / t=2) must keep verifying.
#[test]
fn verify_password_accepts_a_hash_made_with_the_old_default_cost() {
    use argon2::password_hash::{PasswordHasher, SaltString};
    use argon2::{Algorithm, Argon2, Params, Version};

    let old_default = Argon2::new(
        Algorithm::Argon2id,
        Version::V0x13,
        Params::new(19_456, 2, 1, None).unwrap(),
    );
    let salt = SaltString::from_b64("cGVwcGVyc2FsdHNhbHQ").unwrap();
    let legacy_hash = old_default
        .hash_password(b"minha-senha-123", &salt)
        .unwrap()
        .to_string();
    assert!(legacy_hash.contains("m=19456"));

    assert!(verify_password("minha-senha-123", &legacy_hash));
    assert!(!verify_password("senha-errada", &legacy_hash));
}

/// New hashes use the bounded (lower-memory) cost.
#[test]
fn hash_password_writes_the_bounded_cost() {
    let hash = hash_password("minha-senha-123");
    assert!(
        hash.contains("m=7168"),
        "new hashes should carry the reduced argon2 memory cost: {hash}"
    );
}
