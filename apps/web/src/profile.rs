use leptos::prelude::*;
use serde::{Deserialize, Serialize};

/// Kept outside `client/` (gated under `hydrate`) because `home.rs`/`room.rs`
/// reference this type in signatures that must also compile under `ssr`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Profile {
    pub nick: String,
    pub color: String,
}

impl Default for Profile {
    fn default() -> Self {
        Self {
            nick: String::new(),
            color: crate::components::palette::DEFAULT_COLOR.to_string(),
        }
    }
}

// Deliberately has no password field.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RecentRoom {
    pub code: String,
    pub name: String,
}

/// Shared by home.rs and room.rs — both start with the SSR default and only
/// apply the real localStorage value after mount, or hydration mismatches
/// break signal bindings that depend on it (e.g. the selected color swatch).
#[cfg(not(feature = "hydrate"))]
pub fn load_profile_after_mount(_set_nick: WriteSignal<String>, _set_color: WriteSignal<String>) {}

#[cfg(feature = "hydrate")]
pub fn load_profile_after_mount(set_nick: WriteSignal<String>, set_color: WriteSignal<String>) {
    use leptos::task::spawn_local;

    spawn_local(async move {
        let profile = crate::client::storage::load_profile();
        set_nick.set(profile.nick);
        set_color.set(profile.color);
    });
}
