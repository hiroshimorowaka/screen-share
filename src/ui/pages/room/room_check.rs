use leptos::prelude::*;

#[cfg(not(feature = "hydrate"))]
pub(super) fn start_room_check(
    _room_code: String,
    _authenticated: ReadSignal<bool>,
    _set_room_exists: WriteSignal<Option<bool>>,
    _set_room_name: WriteSignal<Option<String>>,
) {
}

#[cfg(feature = "hydrate")]
pub(super) fn start_room_check(
    room_code: String,
    authenticated: ReadSignal<bool>,
    set_room_exists: WriteSignal<Option<bool>>,
    set_room_name: WriteSignal<Option<String>>,
) {
    use leptos::task::spawn_local;

    use crate::ui::client::rooms_api::check_room;

    spawn_local(async move {
        let result = check_room(&room_code).await;
        // A pending session may already have authenticated while this check
        // was in flight — ignore the result in that case.
        if authenticated.get_untracked() {
            return;
        }
        match result {
            Some(status) if status.exists => {
                set_room_name.set(status.name);
                set_room_exists.set(Some(true));
            }
            Some(_) => set_room_exists.set(Some(false)),
            None => set_room_exists.set(Some(true)), // network failed: don't block, let the join attempt through
        }
    });
}
