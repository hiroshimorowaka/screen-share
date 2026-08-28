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
