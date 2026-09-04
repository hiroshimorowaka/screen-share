//! The `/` route: create the lobby's reactive store, wire the
//! after-mount loaders and the create / join handlers, and compose the
//! two panels. Markup lives in `home::components`.

use leptos::prelude::*;

use crate::home::components::create_panel::CreateRoomPanel;
use crate::home::components::join_panel::JoinRoomPanel;
use crate::home::create::{
    create_room_handler, load_last_room_name_after_mount, start_quick_share_after_mount,
};
use crate::home::join::join_room_handler;
use crate::home::recent::{load_recent_rooms_after_mount, prune_recent_rooms};
use crate::home::HomeState;

#[component]
pub fn HomePage() -> impl IntoView {
    let state = HomeState::new();
    provide_context(state);

    crate::profile::load_profile_after_mount(state.set_nick, state.set_color);
    load_last_room_name_after_mount(state.set_room_name);
    load_recent_rooms_after_mount(state.set_recent_rooms);
    prune_recent_rooms(state.set_recent_rooms, state.set_live_rooms);

    let create_room = create_room_handler(
        state.nick,
        state.color,
        state.room_name,
        state.password,
        state.public_room,
        state.set_status,
        state.set_submitting,
    );
    start_quick_share_after_mount(state.set_status, state.set_submitting);
    let join_room = join_room_handler(state.join_input, state.set_join_status);

    // The one live element on the lobby: a mono readout of how many of the
    // recent rooms this browser knows about are currently up (`live_rooms`
    // only holds the ones that answered the prune check).
    let lobby_readout = move || {
        let live = state.live_rooms.get();
        let up = state
            .recent_rooms
            .get()
            .iter()
            .filter(|room| live.contains(&room.code))
            .count();
        match up {
            0 => String::new(),
            1 => "1 sala recente no ar".to_string(),
            n => format!("{n} salas recentes no ar"),
        }
    };

    view! {
        <div class="lobby">
            <header class="lobby__bar">
                <span class="wordmark">"screenshare"<span class="wordmark__dot"></span></span>
                <span class="lobby__readout" class:hidden=move || lobby_readout().is_empty()>
                    {lobby_readout}
                </span>
            </header>

            <section class="hero">
                <h1 class="hero__title">"Compartilhe sua tela com o grupo."</h1>
                <p class="hero__lead">
                    "Uma sala onde qualquer um transmite quando quiser — sem host, sem instalar nada."
                </p>
            </section>

            <div class="lobby__cards">
                <CreateRoomPanel on_submit=create_room/>
                <JoinRoomPanel on_submit=join_room/>
            </div>
        </div>
    }
}
