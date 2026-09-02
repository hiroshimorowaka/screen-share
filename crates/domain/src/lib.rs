//! Browser- and framework-free logic used by the web app's `session`
//! runtime.
//!
//! Depends on nothing — no `web-sys`, no `serde`, no async, no Leptos — so
//! every item here is unit-tested natively (`cargo test -p
//! screen-share-domain`) without the headless-browser harness the rest of
//! `apps/web`'s browser code needs. Anything that has to touch an
//! `RtcPeerConnection`, a timer, or the DOM stays in `apps/web`; this
//! crate only decides *what* those wrappers should do.

pub mod backoff;
pub mod sdp;
