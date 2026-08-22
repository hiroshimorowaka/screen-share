use std::collections::HashMap;

use leptos::prelude::*;

#[cfg(not(feature = "hydrate"))]
pub fn load_recent_rooms_after_mount(_set_recent_rooms: WriteSignal<Vec<crate::ui::profile::RecentRoom>>) {}

#[cfg(feature = "hydrate")]
pub fn load_recent_rooms_after_mount(set_recent_rooms: WriteSignal<Vec<crate::ui::profile::RecentRoom>>) {
    use leptos::task::spawn_local;

    spawn_local(async move {
        set_recent_rooms.set(crate::ui::client::storage::load_recent_rooms());
    });
}

#[cfg(not(feature = "hydrate"))]
pub fn prune_recent_rooms(
    _set_recent_rooms: WriteSignal<Vec<crate::ui::profile::RecentRoom>>,
    _set_member_counts: WriteSignal<HashMap<String, usize>>,
) {
}

#[cfg(feature = "hydrate")]
pub fn prune_recent_rooms(
    set_recent_rooms: WriteSignal<Vec<crate::ui::profile::RecentRoom>>,
    set_member_counts: WriteSignal<HashMap<String, usize>>,
) {
    use leptos::task::spawn_local;

    use crate::ui::client::{rooms_api::check_room, storage::remove_recent_room};

    for room in crate::ui::client::storage::load_recent_rooms() {
        let code = room.code.clone();
        spawn_local(async move {
            if let Some(status) = check_room(&code).await {
                if status.exists {
                    if let Some(count) = status.member_count {
                        set_member_counts.update(|counts| {
                            counts.insert(code.clone(), count);
                        });
                    }
                } else {
                    remove_recent_room(&code);
                    set_recent_rooms.update(|rooms| rooms.retain(|r| r.code != code));
                }
            }
        });
    }
}
