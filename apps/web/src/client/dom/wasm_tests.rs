//! Browser (`wasm32`) tests for `client::dom` — the listener/timer
//! registrations must be gone once the owning reactive scope is cleaned
//! up. Split into its own file so `.cargo/mutants.toml`'s
//! `**/*_wasm_tests.rs` exclusion covers it.

use std::cell::Cell;
use std::rc::Rc;

use leptos::prelude::Owner;
use wasm_bindgen::prelude::Closure;
use wasm_bindgen_test::*;

use super::listen_until_cleanup;

wasm_bindgen_test_configure!(run_in_browser);

fn dispatch(event: &str) {
    let window = web_sys::window().unwrap();
    let ev = web_sys::Event::new(event).unwrap();
    window.dispatch_event(&ev).unwrap();
}

#[wasm_bindgen_test]
fn listener_stops_firing_after_the_owner_is_cleaned_up() {
    // A distinctive event name so a stray global listener from another
    // test can't perturb the count.
    const EVENT: &str = "screen-share-dom-cleanup-test";

    let hits = Rc::new(Cell::new(0u32));
    let owner = Owner::new();
    owner.with(|| {
        let hits = hits.clone();
        listen_until_cleanup(
            web_sys::window().unwrap(),
            EVENT,
            Closure::<dyn FnMut()>::new(move || hits.set(hits.get() + 1)),
        );
    });

    dispatch(EVENT);
    assert_eq!(hits.get(), 1, "listener runs while the owner is alive");

    owner.cleanup();

    dispatch(EVENT);
    assert_eq!(
        hits.get(),
        1,
        "listener must be removed once the owner is cleaned up"
    );
}
