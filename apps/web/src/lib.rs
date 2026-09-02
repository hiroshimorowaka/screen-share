// Leptos builds one deeply-nested type per `view!`; `RoomPage`'s is large
// enough (control bar + grid + gate panels) to need this above the default.
#![recursion_limit = "512"]

pub mod app;
pub mod components;
pub mod features;
pub mod session;

#[cfg(feature = "hydrate")]
pub mod infra;
#[cfg(feature = "hydrate")]
pub mod quick_share;

#[cfg(feature = "ssr")]
pub mod http_limits;
#[cfg(feature = "ssr")]
pub mod http_security;

#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    use crate::app::*;
    console_error_panic_hook::set_once();
    leptos::mount::hydrate_body(App);
}
