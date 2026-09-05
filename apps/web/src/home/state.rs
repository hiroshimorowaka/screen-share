//! `HomeState` — every reactive signal the lobby needs, in one `Copy`
//! struct created by [`HomeState::new`] and delivered via
//! `provide_context` so the create / join panels read it without a long
//! prop list.

use std::collections::HashSet;

use leptos::prelude::*;

use crate::profile::RecentRoom;

#[derive(Clone, Copy)]
pub(crate) struct HomeState {
    // --- create panel ---
    pub(crate) nick: ReadSignal<String>,
    pub(crate) set_nick: WriteSignal<String>,
    pub(crate) color: ReadSignal<String>,
    pub(crate) set_color: WriteSignal<String>,
    pub(crate) room_name: ReadSignal<String>,
    pub(crate) set_room_name: WriteSignal<String>,
    pub(crate) password: ReadSignal<String>,
    pub(crate) set_password: WriteSignal<String>,
    pub(crate) public_room: ReadSignal<bool>,
    pub(crate) set_public_room: WriteSignal<bool>,
    pub(crate) status: ReadSignal<String>,
    pub(crate) set_status: WriteSignal<String>,
    pub(crate) submitting: ReadSignal<bool>,
    pub(crate) set_submitting: WriteSignal<bool>,

    // --- join panel + recent rooms ---
    pub(crate) join_input: ReadSignal<String>,
    pub(crate) set_join_input: WriteSignal<String>,
    pub(crate) join_status: ReadSignal<String>,
    pub(crate) set_join_status: WriteSignal<String>,
    pub(crate) recent_rooms: ReadSignal<Vec<RecentRoom>>,
    pub(crate) set_recent_rooms: WriteSignal<Vec<RecentRoom>>,
    /// Which remembered rooms answered the liveness check as still up.
    /// From the server, never persisted; a plain set, not a count map —
    /// the status endpoint exposes no occupancy.
    pub(crate) live_rooms: ReadSignal<HashSet<String>>,
    pub(crate) set_live_rooms: WriteSignal<HashSet<String>>,
}

impl HomeState {
    /// Creates every signal the lobby needs. Signals start at the value
    /// SSR would use (empty/default); the real `localStorage` value is
    /// applied after mount, or Leptos hydration breaks.
    #[must_use]
    pub(crate) fn new() -> Self {
        let (nick, set_nick) = signal(String::new());
        let (color, set_color) = signal(crate::components::palette::DEFAULT_COLOR.to_string());
        let (room_name, set_room_name) = signal(String::new());
        let (password, set_password) = signal(String::new());
        let (public_room, set_public_room) = signal(false);
        let (status, set_status) = signal("Pronto para criar uma sala.".to_string());
        let (submitting, set_submitting) = signal(false);
        let (join_input, set_join_input) = signal(String::new());
        let (join_status, set_join_status) = signal(String::new());
        let (recent_rooms, set_recent_rooms) = signal(Vec::<RecentRoom>::new());
        let (live_rooms, set_live_rooms) = signal(HashSet::<String>::new());

        Self {
            nick,
            set_nick,
            color,
            set_color,
            room_name,
            set_room_name,
            password,
            set_password,
            public_room,
            set_public_room,
            status,
            set_status,
            submitting,
            set_submitting,
            join_input,
            set_join_input,
            join_status,
            set_join_status,
            recent_rooms,
            set_recent_rooms,
            live_rooms,
            set_live_rooms,
        }
    }
}
