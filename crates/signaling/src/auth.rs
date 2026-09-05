use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::{Algorithm, Argon2, Params, Version};

/// Argon2id memory cost, in KiB. `Argon2::default()` asks for 19 MiB per
/// call; joining a room needs no prior auth, so on the 256 MB production VM
/// a burst of wrong-password `JoinRoom`s (each running `verify_password`)
/// could OOM the process and drop every in-memory room. This is OWASP's
/// lowest-memory Argon2id profile — 7 MiB / t=5 / p=1 — which keeps
/// brute-force resistance equivalent to the 19 MiB / t=2 default while
/// cutting the per-call footprint by ~2.7x.
const ARGON2_MEMORY_KIB: u32 = 7_168;
/// Argon2id iteration count — raised from the default `2` to compensate for
/// the reduced [`ARGON2_MEMORY_KIB`], per the same OWASP profile.
const ARGON2_ITERATIONS: u32 = 5;
/// Argon2id parallelism (lanes). One, matching every OWASP profile and the
/// crate default.
const ARGON2_PARALLELISM: u32 = 1;

/// Longest password accepted before hashing. Argon2 salts and stretches the
/// input, so a very long password only wastes CPU/memory on every
/// `CreateRoom` / `JoinRoom` — an amplification vector, and an OWASP auth
/// anti-pattern. 128 is far above any real room password.
pub const MAX_PASSWORD_LEN: usize = 128;

/// The cost-bounded Argon2id hasher used for both hashing and verification.
///
/// Verification honours whatever parameters are embedded in the stored
/// PHC string, so a hash written at a different cost still verifies.
fn hasher() -> Argon2<'static> {
    let params = Params::new(
        ARGON2_MEMORY_KIB,
        ARGON2_ITERATIONS,
        ARGON2_PARALLELISM,
        None,
    )
    .expect("Argon2 parameters are compile-time constants within the crate's accepted ranges");
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
}

pub fn hash_password(password: &str) -> String {
    let salt = SaltString::generate(&mut OsRng);
    hasher()
        .hash_password(password.as_bytes(), &salt)
        .expect("argon2 hashing a valid UTF-8 password should never fail")
        .to_string()
}

pub fn verify_password(password: &str, hash: &str) -> bool {
    let Ok(parsed_hash) = PasswordHash::new(hash) else {
        return false;
    };
    hasher()
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok()
}
