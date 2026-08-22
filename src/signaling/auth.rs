use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;

pub fn hash_password(password: &str) -> String {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .expect("argon2 hashing a valid UTF-8 password should never fail")
        .to_string()
}

pub fn verify_password(password: &str, hash: &str) -> bool {
    let Ok(parsed_hash) = PasswordHash::new(hash) else { return false };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
