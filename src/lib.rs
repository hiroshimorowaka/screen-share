pub mod app;
pub mod pages;
pub mod profile;
pub mod signaling;

#[cfg(feature = "hydrate")]
pub mod client;

#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    use crate::app::*;
    console_error_panic_hook::set_once();
    leptos::mount::hydrate_body(App);
}
