// Leptos builds one deeply-nested type per `view!`; `RoomPage`'s is large
// enough (control bar + grid + gate panels) to need this above the default.
#![recursion_limit = "512"]
// Keep functions and components small: flag any that grow past ~100 lines
// or take more than seven parameters. Existing offenders carry an
// item-level `#[allow]` naming the refactor step that retires them; a new
// warning here means a function needs splitting, not another allow.
#![warn(clippy::too_many_lines)]
#![warn(clippy::too_many_arguments)]

pub mod app;
pub mod components;
pub mod features;
pub mod session;

#[cfg(feature = "hydrate")]
pub mod infra;
#[cfg(feature = "hydrate")]
pub mod quick_share;

#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    use crate::app::*;
    console_error_panic_hook::set_once();
    leptos::mount::hydrate_body(App);
}
