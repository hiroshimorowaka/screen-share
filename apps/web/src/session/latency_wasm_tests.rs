//! Browser (`wasm32`) tests for `latency::round_trip_ms_since`.
//! Split into its own file so `.cargo/mutants.toml`'s
//! `**/*_wasm_tests.rs` exclusion covers it (cargo-mutants can't
//! evaluate the `cfg` and treats `#[wasm_bindgen_test]` fns as code).

use wasm_bindgen_test::*;

use super::round_trip_ms_since;

wasm_bindgen_test_configure!(run_in_browser);

fn now() -> f64 {
    web_sys::window().unwrap().performance().unwrap().now()
}

#[wasm_bindgen_test]
fn a_future_timestamp_clamps_to_zero_rather_than_going_negative() {
    assert_eq!(round_trip_ms_since(now() + 10_000.0), Some(0));
}

#[wasm_bindgen_test]
fn a_past_timestamp_yields_the_elapsed_milliseconds() {
    let ms = round_trip_ms_since(0.0).expect("performance.now() is available in a browser");
    assert!(ms > 0, "measurable time has passed since the page loaded");
}
