//! The authenticated room view: the stage header, the participant grid,
//! and the control bar. Reads `RoomState` / `RoomSession` from context
//! (provided by `pages::room`) and owns the one-time grid/fullscreen
//! effect wiring the grid markup depends on.

use leptos::prelude::*;

use super::participant::member_cards;
use super::participant_grid::setup_adaptive_grid;
use super::room_controls::RoomControls;
use super::stage_header::StageHeader;
use crate::room::media::{share_supported, sharing_can_have_audio};
use crate::room::media_controls::setup_fullscreen_autohide_controls;
use crate::room::{RoomSession, RoomState};

#[component]
pub(crate) fn Stage(
    /// Route code, for the invite link and persisting a join.
    room_code: String,
    /// The per-tab imperative session (not `Send`, so a prop, not context).
    conn: RoomSession,
) -> impl IntoView {
    let state = expect_context::<RoomState>();

    setup_adaptive_grid(
        state.members,
        state.hide_idle,
        state.own_preview_hidden,
        state.is_sharing,
        state.expanded,
    );
    setup_fullscreen_autohide_controls();

    let can_share = share_supported();
    // The desktop shell captures system audio; a plain browser tab can
    // capture its own tab audio through the picker. Either way the
    // audio-quality / mute controls apply — they only stay hidden on a
    // browser that can't screen-share at all.
    let sharing_has_audio = sharing_can_have_audio();

    view! {
        <div
            class="room-page"
            class:hidden=move || !state.authenticated.get()
            class:chrome-hidden=move || !state.controls_visible.get()
        >
            <StageHeader
                room_code=room_code.clone()
                can_share=can_share
                sharing_has_audio=sharing_has_audio
            />
            <div
                id="member-grid"
                class="grid"
                class:grid--focused=move || state.expanded.get().is_some()
            >
                {member_cards(conn.clone())}
            </div>
            <RoomControls room_code=room_code can_share=can_share conn=conn/>
        </div>
    }
}
