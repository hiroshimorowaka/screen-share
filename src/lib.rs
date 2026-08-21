#![recursion_limit = "256"]

pub mod signaling;
pub mod ui;

#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    use crate::ui::app::*;
    console_error_panic_hook::set_once();
    leptos::mount::hydrate_body(App);
}
