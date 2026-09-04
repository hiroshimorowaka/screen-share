//! Server-side render snapshots — cheap coverage of what a component
//! actually emits as HTML, with no browser in the loop. Runs in the normal
//! `cargo test --features ssr` suite.
//!
//! These assert on stable substrings (class names, the text a component is
//! handed), not the whole HTML blob, which churns on every Leptos bump.

#![cfg(feature = "ssr")]

use leptos::prelude::*;
use screen_share::components::ui::status_message::StatusMessage;

/// Render a view to its server HTML string under a fresh reactive owner
/// (signals created in the closure need one).
fn render<V: IntoView + 'static>(view: impl FnOnce() -> V + 'static) -> String {
    let owner = Owner::new();
    owner.with(|| view().into_view().to_html())
}

#[test]
fn status_message_marks_an_error_sentence_with_the_error_modifier_class() {
    let html = render(|| {
        let (status, _) = signal("Não foi possível conectar.".to_string());
        view! { <StatusMessage status /> }
    });

    assert!(html.contains("status-text--error"), "html was: {html}");
    assert!(html.contains("Não foi possível conectar."));
}

#[test]
fn status_message_leaves_a_non_error_sentence_unmodified() {
    let html = render(|| {
        let (status, _) = signal("Conectado.".to_string());
        view! { <StatusMessage status /> }
    });

    assert!(!html.contains("status-text--error"), "html was: {html}");
    assert!(html.contains("Conectado."));
}
