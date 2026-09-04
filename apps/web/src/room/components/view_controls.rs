//! The view-preference buttons: hide idle members, hide own preview, and
//! the transmission menu (video mode / audio preset / mute). Owns the
//! video-mode and audio-preset handlers; reads state from context.

use leptos::prelude::*;

use super::transmission_menu::TransmissionMenu;
use crate::components::ui::icons::{icon_eye_off, icon_video_off};
use crate::room::audio::set_audio_preset_handler;
use crate::room::video_mode::set_video_mode_handler;
use crate::room::{RoomSession, RoomState};

#[component]
pub(super) fn ViewControls(
    /// The per-tab imperative session (not `Send`, so a prop, not context).
    conn: RoomSession,
) -> impl IntoView {
    let state = expect_context::<RoomState>();
    let RoomState {
        my_peer_id,
        is_sharing,
        own_preview_hidden,
        audio_muted,
        share_has_audio,
        audio_preset,
        video_mode,
        expanded,
        hide_idle,
        ..
    } = state;

    let set_audio_preset = set_audio_preset_handler(conn.clone(), audio_preset);
    let set_video_mode = set_video_mode_handler(conn, video_mode);

    // Hiding your own preview while it is the expanded card would leave
    // the grid focused on a card that just left it — drop focus too.
    let toggle_own_preview = move |_| {
        let now_hidden = !own_preview_hidden.get_untracked();
        own_preview_hidden.set(now_hidden);
        if now_hidden && expanded.get_untracked() == my_peer_id.get_untracked() {
            expanded.set(None);
        }
    };

    view! {
        <button
            class="icon-btn icon-btn--neutral"
            class:icon-btn--active=hide_idle
            title=move || if hide_idle.get() { "Mostrar todo mundo" } else { "Ocultar quem não está transmitindo" }
            aria-label="Ocultar quem não está transmitindo"
            on:click=move |_| hide_idle.update(|v| *v = !*v)
        >
            {icon_eye_off}
        </button>
        <button
            class="icon-btn icon-btn--neutral"
            class:icon-btn--active=own_preview_hidden
            class:hidden=move || !is_sharing.get()
            title=move || if own_preview_hidden.get() { "Mostrar meu preview" } else { "Esconder meu preview" }
            aria-label="Esconder meu preview"
            on:click=toggle_own_preview
        >
            {icon_video_off}
        </button>
        <div class="control-group__menu" class:hidden=move || !is_sharing.get()>
            <TransmissionMenu
                video_mode=video_mode
                on_video_mode=set_video_mode
                audio_preset=audio_preset
                on_audio_preset=set_audio_preset
                has_audio=share_has_audio
                audio_muted=audio_muted
            />
        </div>
    }
}
