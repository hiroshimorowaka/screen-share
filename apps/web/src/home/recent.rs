use std::collections::HashSet;

use leptos::prelude::*;

#[cfg(not(feature = "hydrate"))]
pub fn load_recent_rooms_after_mount(
    _set_recent_rooms: WriteSignal<Vec<crate::profile::RecentRoom>>,
) {
}

#[cfg(feature = "hydrate")]
pub fn load_recent_rooms_after_mount(
    set_recent_rooms: WriteSignal<Vec<crate::profile::RecentRoom>>,
) {
    use leptos::task::spawn_local;

    spawn_local(async move {
        set_recent_rooms.set(crate::client::storage::load_recent_rooms());
    });
}

#[cfg(not(feature = "hydrate"))]
pub fn prune_recent_rooms(
    _set_recent_rooms: WriteSignal<Vec<crate::profile::RecentRoom>>,
    _set_live_rooms: WriteSignal<HashSet<String>>,
) {
}

/// Checks each remembered room against `GET /api/rooms/:code`: drops the
/// ones that no longer exist, and records the ones still up in
/// `live_rooms`. The status endpoint exposes no member count, so this
/// tracks liveness only, not occupancy.
#[cfg(feature = "hydrate")]
pub fn prune_recent_rooms(
    set_recent_rooms: WriteSignal<Vec<crate::profile::RecentRoom>>,
    set_live_rooms: WriteSignal<HashSet<String>>,
) {
    use leptos::task::spawn_local;

    use crate::client::{rooms_api::check_room, storage::remove_recent_room};

    for room in crate::client::storage::load_recent_rooms() {
        let code = room.code.clone();
        spawn_local(async move {
            if let Some(status) = check_room(&code).await {
                if status.exists {
                    set_live_rooms.update(|live| {
                        live.insert(code.clone());
                    });
                } else {
                    remove_recent_room(&code);
                    set_recent_rooms.update(|rooms| rooms.retain(|r| r.code != code));
                }
            }
        });
    }
}
