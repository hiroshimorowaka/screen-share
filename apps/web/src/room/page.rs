//! The `/r/:code` route shell: create the room's reactive store and
//! imperative session, provide both via context, wire the one-time
//! connection / effect setup, and render the pre-auth `<RoomGate>` plus
//! the authenticated `<Stage>`. All markup lives in `room::components`.

use leptos::prelude::*;
use leptos_router::hooks::use_params_map;

use crate::room::components::gate::RoomGate;
use crate::room::components::stage::Stage;
use crate::room::share_effects::{setup_quick_share_auto_flow, setup_share_side_effects};
use crate::room::{
    adopt_pending_session, latency, reconnect, room_check, setup_room_connection, touch,
    RoomSession, RoomState,
};

#[component]
pub fn RoomPage() -> impl IntoView {
    let params = use_params_map();
    let code = move || params.read().get("code").unwrap_or_default();
    let initial_code = params.read_untracked().get("code").unwrap_or_default();

    // Starts at the SSR default value; the real one (localStorage) only
    // arrives after mount, or hydration of the selected color swatch breaks.
    let (nick, set_nick) = signal(String::new());
    let (color, set_color) = signal(crate::components::palette::DEFAULT_COLOR.to_string());
    crate::profile::load_profile_after_mount(set_nick, set_color);
    let (password, set_password) = signal(String::new());

    // Every reactive signal the room view and its runtime share (see
    // `room::state`). Created once, `provide_context`'d so the components
    // read it without a prop.
    let state = RoomState::new();
    provide_context(state);

    // `RoomSession` holds `Rc<RefCell<…>>` (not `Send + Sync`), so it is
    // threaded as a plain value / component prop, never `provide_context`'d
    // the way `RoomState`'s `Copy` signal handles are.
    let conn = RoomSession::new();

    let join_room = setup_room_connection(initial_code.clone(), conn.clone(), state);

    adopt_pending_session(
        initial_code.clone(),
        conn.clone(),
        state,
        state.set_requires_password,
    );

    // Reloading the page while still in a room shouldn't drop back to the
    // nick/password gate — rejoin silently with whatever this same tab used
    // last time, same as `adopt_pending_session` does for the creator's own
    // first load. Only runs if that didn't already authenticate us.
    #[cfg(feature = "hydrate")]
    if !state.authenticated.get_untracked() {
        if let Some(stored) = crate::client::storage::load_room_session(&initial_code) {
            join_room(stored.nick, stored.color, stored.password);
        }
    }

    // The desktop tray's quick-share flow (a no-op unless the URL carries
    // the `quick_share` flag) — see `room::share_effects`.
    setup_quick_share_auto_flow(
        conn.clone(),
        initial_code.clone(),
        state.authenticated,
        state,
        state.set_status,
        state.my_peer_id,
        state.expanded,
    );

    room_check::start_room_check(
        initial_code.clone(),
        state.authenticated,
        state.set_room_exists,
        state.set_requires_password,
    );

    touch::setup_touch_signal(state.set_is_touch);
    latency::setup_ping_loop(conn.clone());
    // On leaving the room, tear down every peer connection, its callbacks,
    // and the Auto-quality polls — otherwise a leaked callback keeps the
    // whole session alive in memory.
    reconnect::drop_peers_on_cleanup(conn.clone());

    // The audio self-test, the outgoing-mute toggle, and copying the
    // invite link on share start — see `room::share_effects`.
    setup_share_side_effects(
        conn.clone(),
        initial_code.clone(),
        state,
        state.invite_copied,
    );

    view! {
        <RoomGate
            code=Signal::derive(code)
            authenticated=state.authenticated
            room_exists=state.room_exists
            requires_password=state.requires_password
            nick=nick
            set_nick=set_nick
            color=color
            set_color=set_color
            password=password
            set_password=set_password
            status=state.status
            set_status=state.set_status
            room_code=initial_code.clone()
            on_join=join_room.clone()
        />
        <Stage room_code=initial_code conn=conn/>
    }
}
