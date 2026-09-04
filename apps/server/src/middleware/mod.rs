//! SSR-only HTTP layers, moved out of the (now library-only) `apps/web`
//! crate: the CSP + per-request nonce ([`security`]), and the DoS guards
//! ([`limits`] — request timeout + global concurrency cap + per-IP rate
//! limit).
pub mod limits;
pub mod security;
