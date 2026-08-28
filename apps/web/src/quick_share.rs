//! Plumbing for the desktop tray's "share now" action: create a room with
//! a random name, auto-join it, and auto-start sharing without ever
//! showing a form — driven entirely by a `quick_share=1` query param the
//! desktop shell appends to the URL it loads. A plain browser tab never
//! sets this param, so none of this affects normal usage.

use leptos::prelude::*;
use leptos_router::hooks::use_query_map;

const QUICK_SHARE_QUERY_PARAM: &str = "quick_share";

/// Whether the current URL asks for the quick-share flow. Read once at
/// mount time (`get_untracked`) — this is a one-shot trigger, not a value
/// the page should keep reacting to.
pub fn requested() -> bool {
    use_query_map()
        .get_untracked()
        .get(QUICK_SHARE_QUERY_PARAM)
        .as_deref()
        == Some("1")
}

/// The room page URL that keeps the quick-share flag alive across the
/// home page's post-creation redirect, so the room page knows to
/// auto-start sharing instead of waiting for a click.
pub fn room_path_with_flag(room_code: &str) -> String {
    format!("/r/{room_code}?{QUICK_SHARE_QUERY_PARAM}=1")
}

fn random_suffix() -> u32 {
    (js_sys::Math::random() * 9000.0) as u32 + 1000
}

pub fn random_nick() -> String {
    format!("Convidado {}", random_suffix())
}

pub fn random_room_name() -> String {
    format!("Sala rápida {}", random_suffix())
}
