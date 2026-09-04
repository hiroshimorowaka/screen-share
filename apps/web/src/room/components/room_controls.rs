//! The floating control bar: the sharing-controls cluster
//! (`SharingControls`) plus the leave-or-stop-watching button. Owns the
//! pointer-idle auto-hide wiring and the leave handler; reads state from
//! context.

use leptos::prelude::*;

use super::participant_grid::setup_auto_hide_controls;
use super::sharing_controls::SharingControls;
use super::view_controls::ViewControls;
use crate::components::ui::icons::{icon_log_out, icon_screen_off};
use crate::room::watch::leave_or_stop_watching_handler;
use crate::room::{RoomSession, RoomState};

#[component]
pub(super) fn RoomControls(
    room_code: String,
    /// This browser can screen-share at all (`getDisplayMedia` present).
    can_share: bool,
    /// The per-tab imperative session (not `Send`, so a prop, not context).
    conn: RoomSession,
) -> impl IntoView {
    let state = expect_context::<RoomState>();
    let RoomState {
        my_peer_id,
        watching,
        expanded,
        controls_visible,
        is_touch,
        ..
    } = state;

    let leave_or_stop_watching =
        leave_or_stop_watching_handler(conn.clone(), watching, expanded, my_peer_id, room_code);
    let (pause_hide_controls, resume_hide_controls) =
        setup_auto_hide_controls(controls_visible, is_touch, expanded);

    view! {
        <div
            class="room-controls"
            class:room-controls--hidden=move || !controls_visible.get()
            on:mouseenter=move |_| pause_hide_controls()
            on:mouseleave=move |_| resume_hide_controls()
        >
            <div class="control-group">
                <SharingControls can_share=can_share conn=conn.clone()/>
                <ViewControls conn=conn/>
            </div>
            <div class="control-group control-group--danger">
                <button
                    class="icon-btn icon-btn--danger"
                    title=move || if expanded.get().is_some() { "Parar de assistir" } else { "Sair da sala" }
                    aria-label=move || if expanded.get().is_some() { "Parar de assistir" } else { "Sair da sala" }
                    on:click=leave_or_stop_watching
                >
                    {move || if expanded.get().is_some() { icon_screen_off().into_any() } else { icon_log_out().into_any() }}
                </button>
            </div>
        </div>
    }
}
