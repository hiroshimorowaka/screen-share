// Leptos builds one deeply-nested type per `view!`; the room view's is
// large enough (control bar + grid + gate panels) to need this above the
// default.
#![recursion_limit = "512"]
// Keep functions and components small: flag any that grow past ~100 lines
// or take more than seven parameters. A new warning here means a function
// needs splitting, not another allow.
#![warn(clippy::too_many_lines)]
#![warn(clippy::too_many_arguments)]

pub mod app;
pub mod components;
pub mod home;
pub mod not_found;
pub mod profile;
pub mod room;

#[cfg(feature = "hydrate")]
pub mod client;

#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    console_error_panic_hook::set_once();
    leptos::mount::hydrate_body(crate::app::App);
}
