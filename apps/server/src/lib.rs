//! `screen-share-server` — the Axum host for the screen-share app.
//!
//! Renders the `screen_share` Leptos UI library server-side and runs the
//! signaling relay (`crates/signaling`). All meaning of the signaling
//! messages lives in the browser (`screen_share`'s `hydrate` build); this
//! crate only routes them and serves HTML/assets.
//!
//! The binary (`src/main.rs`) is a thin bootstrap over this library:
//! [`config::ServerConfig::from_env`] reads the environment,
//! [`router::build`] composes the full service, `main` binds and serves.

// Leptos builds one deeply-nested type per `view!`; rendering the lib's
// `RoomPage` route monomorphises it here, deep enough to need this above
// the default (same attribute on the lib).
#![recursion_limit = "512"]
// Same small-function gate as the UI lib (this is a separate crate to the
// linter).
#![warn(clippy::too_many_lines)]
#![warn(clippy::too_many_arguments)]

#[cfg(feature = "ssr")]
pub mod config;
#[cfg(feature = "ssr")]
pub mod middleware;
#[cfg(feature = "ssr")]
pub mod router;
