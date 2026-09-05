//! The stage header: connection lamp, room name / member count, the
//! "sharing" and audio chips, the transient status sentence, and the
//! invite button. Reads `RoomState` from context.

use leptos::prelude::*;
use screen_share_domain::status::{status_kind, StatusKind};
use screen_share_protocol::MAX_MEMBERS;

use crate::components::ui::icons::{icon_check, icon_link};
use crate::room::audio_health::AudioHealth;
use crate::room::invite::invite_click_handler;
use crate::room::RoomState;

#[component]
pub(super) fn StageHeader(
    room_code: String,
    /// This browser can screen-share at all (`getDisplayMedia` present).
    can_share: bool,
    /// Screen-share on this platform can carry audio (drives the chip copy).
    sharing_has_audio: bool,
) -> impl IntoView {
    let state = expect_context::<RoomState>();
    let RoomState {
        status,
        room_name,
        members,
        is_sharing,
        audio_muted,
        share_has_audio,
        audio_health,
        invite_copied,
        is_touch,
        ..
    } = state;

    let invite_click = invite_click_handler(room_code, invite_copied);
    let lamp_class = move || format!("lamp lamp--{}", status_kind(&status.get()).as_css_suffix());

    view! {
        <div class="stage-header">
            <span class=lamp_class></span>
            <span class="status-row__meta">{move || room_name.get().unwrap_or_default()}</span>
            <span class="room-member-count">{move || format!("{}/{}", members.get().len(), MAX_MEMBERS)}</span>
            <span
                class="share-chip"
                class:hidden=move || !is_sharing.get()
                aria-live="polite"
            >
                <span class="share-chip__dot" aria-hidden="true"></span>
                "Compartilhando"
            </span>
            <AudioChip
                is_sharing=is_sharing
                audio_muted=audio_muted
                share_has_audio=share_has_audio
                audio_health=audio_health
                sharing_has_audio=sharing_has_audio
            />
            // Surface the status sentence only while something is off or
            // in progress (reconnecting, an error) — the steady "Conectado."
            // state stays silent, represented by the lamp alone.
            <span
                class="stage-header__status"
                class:stage-header__status--error=move || status_kind(&status.get()) == StatusKind::Error
                class:hidden=move || matches!(status_kind(&status.get()), StatusKind::Idle | StatusKind::Live)
            >
                {move || status.get()}
            </span>
            <span class="status-row__spacer"></span>
            <button
                class="invite-btn"
                class:invite-btn--copied=invite_copied
                title="Copiar link de convite da sala"
                aria-label="Copiar link de convite da sala"
                on:click=invite_click
            >
                {move || if invite_copied.get() { icon_check().into_any() } else { icon_link().into_any() }}
                <span>{move || if invite_copied.get() { "Link copiado!" } else { "Convidar" }}</span>
            </button>
            // On a phone this isn't an error — no mobile browser can
            // screen-share — so it reads as a plain note, not red.
            <span
                class="status-text"
                class:status-text--error=move || !is_touch.get()
                class:hidden=move || can_share
            >
                {move || {
                    if is_touch.get() {
                        "Compartilhar tela não é possível neste aparelho — você pode assistir normalmente."
                    } else {
                        "Seu navegador não suporta compartilhar tela — você ainda pode assistir."
                    }
                }}
            </span>
        </div>
    }
}

/// The sharer's own outgoing-audio chip: on/off/muted, plus a red or amber
/// signal when the audio self-test found something wrong. Split out of
/// `StageHeader` to keep that component under the line-count lint.
#[component]
fn AudioChip(
    is_sharing: ReadSignal<bool>,
    audio_muted: RwSignal<bool>,
    share_has_audio: RwSignal<bool>,
    audio_health: RwSignal<AudioHealth>,
    /// Screen-share on this platform can carry audio at all (drives the
    /// "Áudio indisponível" copy).
    sharing_has_audio: bool,
) -> impl IntoView {
    view! {
        <span
            class="audio-chip"
            class:audio-chip--muted=audio_muted
            class:audio-chip--error=move || audio_health.get() == AudioHealth::CaptureFailed
            class:hidden=move || !is_sharing.get()
            aria-live="polite"
        >
            <span
                class="audio-chip__dot"
                aria-hidden="true"
                class:hidden=move || {
                    !(audio_muted.get()
                        || share_has_audio.get()
                        || audio_health.get() == AudioHealth::CaptureFailed)
                }
            ></span>
            <span>
                {move || {
                    if !sharing_has_audio {
                        // A browser too old for `getDisplayMedia` (which
                        // also can't share video at all).
                        "Áudio indisponível"
                    } else if audio_muted.get() {
                        "Áudio mudo"
                    } else if share_has_audio.get() {
                        "Áudio ligado"
                    } else {
                        // No audio track at all — either the sharer left
                        // audio out on purpose (nothing wrong), or the
                        // capture was requested and failed outright
                        // (`AudioHealth::CaptureFailed`). Either way there
                        // is nothing to hear, so the label reads the
                        // same; the red dot and the "!" tooltip above are
                        // what tell the two apart.
                        "Áudio desligado"
                    }
                }}
            </span>
            // The audio self-test's diagnostic, folded into the chip as a
            // hover-for-detail "!" instead of a loose red sentence
            // elsewhere in the header. Only meaningful while a real
            // (unmuted) audio track is being sent.
            <span
                class="audio-chip__warn"
                class:hidden=move || audio_health.get().warning().is_none() || audio_muted.get()
                tabindex="0"
                role="note"
                aria-label=move || audio_health.get().warning().unwrap_or_default()
            >
                "!"
                <span class="audio-chip__warn-tip" role="tooltip">
                    {move || audio_health.get().warning().unwrap_or_default()}
                </span>
            </span>
        </span>
    }
}

#[cfg(all(test, feature = "ssr"))]
#[path = "stage_header_tests.rs"]
mod tests;
