//! The stream-action buttons: back-to-grid (touch only), start/stop
//! share, and switch source. Owns the share/switch handlers; reads state
//! from context. The view-preference buttons are in `view_controls`.

use leptos::prelude::*;

use crate::components::ui::icons::{icon_minimize, icon_monitor, icon_screen_off, icon_switch};
use crate::room::media::{share_toggle_handler, switch_source_handler, BrowserDisplayCapture};
use crate::room::{RoomSession, RoomState};

#[component]
pub(super) fn SharingControls(
    /// This browser can screen-share at all (`getDisplayMedia` present).
    can_share: bool,
    /// The per-tab imperative session (not `Send`, so a prop, not context).
    conn: RoomSession,
) -> impl IntoView {
    let state = expect_context::<RoomState>();
    let RoomState {
        set_status,
        my_peer_id,
        is_sharing,
        set_is_sharing,
        own_preview_hidden,
        audio_muted,
        share_generation,
        video_mode,
        expanded,
        is_touch,
        ..
    } = state;

    let toggle_share = share_toggle_handler(
        conn.clone(),
        is_sharing,
        set_is_sharing,
        own_preview_hidden,
        set_status,
        my_peer_id,
        expanded,
    );
    let switch_source = switch_source_handler(
        BrowserDisplayCapture,
        conn,
        set_is_sharing,
        own_preview_hidden,
        set_status,
        my_peer_id,
        expanded,
        audio_muted.read_only(),
        video_mode.read_only(),
        share_generation,
    );

    view! {
        // Touch has no tap-outside-the-video to leave focus with; this is
        // the way back to the grid. Desktop clicks the focused card
        // itself, so it only shows on touch.
        <button
            class="icon-btn icon-btn--neutral"
            class:hidden=move || !(is_touch.get() && expanded.get().is_some())
            title="Voltar para a grade"
            aria-label="Voltar para a grade"
            on:click=move |_| expanded.set(None)
        >
            {icon_minimize}
        </button>
        <button
            class="icon-btn"
            class:icon-btn--danger=is_sharing
            class:icon-btn--neutral=move || !is_sharing.get()
            class:hidden=move || !can_share
            title=move || if is_sharing.get() { "Parar de compartilhar" } else { "Compartilhar minha tela" }
            aria-label="Compartilhar ou parar de compartilhar minha tela"
            on:click=toggle_share.clone()
        >
            {move || if is_sharing.get() { icon_screen_off().into_any() } else { icon_monitor().into_any() }}
        </button>
        <button
            class="icon-btn icon-btn--neutral"
            class:hidden=move || !is_sharing.get()
            title="Trocar a tela ou janela compartilhada"
            aria-label="Trocar a tela ou janela compartilhada"
            on:click=switch_source.clone()
        >
            {icon_switch}
        </button>
    }
}
