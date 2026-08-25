use leptos::prelude::*;

#[cfg(not(feature = "hydrate"))]
pub(super) fn start_room_check(
    _room_code: String,
    _authenticated: ReadSignal<bool>,
    _set_room_exists: WriteSignal<Option<bool>>,
    _set_room_name: WriteSignal<Option<String>>,
    _set_requires_password: WriteSignal<bool>,
) {
}

#[cfg(feature = "hydrate")]
pub(super) fn start_room_check(
    room_code: String,
    authenticated: ReadSignal<bool>,
    set_room_exists: WriteSignal<Option<bool>>,
    set_room_name: WriteSignal<Option<String>>,
    set_requires_password: WriteSignal<bool>,
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
                set_requires_password.set(status.requires_password.unwrap_or(false));
                set_room_exists.set(Some(true));
            }
            Some(_) => set_room_exists.set(Some(false)),
            // Network failed: don't block, let the join attempt through —
            // but assume a password may be needed rather than hiding the
            // field and having the join fail with no way to enter one.
            None => {
                set_requires_password.set(true);
                set_room_exists.set(Some(true));
            }
        }
    });
}
