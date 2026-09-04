//! Browser-only building blocks — every submodule here is
//! `#[cfg(feature = "hydrate")]` (the whole tree is, via `lib.rs`).

pub(crate) mod desktop_bridge;
pub mod dom;
pub mod rooms_api;
pub(crate) mod seam;
pub mod session;
pub mod socket;
pub mod storage;
pub mod webrtc;
