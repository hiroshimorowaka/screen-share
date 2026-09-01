//! The sharer's transmission settings — video mode, audio quality, and
//! outgoing mute — folded into one popover on the control bar, instead of
//! three separate controls competing for width on a small bar. Reveals on
//! hover / keyboard focus, same as [`MenuSelect`](super::menu_select).

use leptos::prelude::*;

use crate::components::icons::{icon_sliders, icon_volume, icon_volume_off};
use crate::features::room::media_controls::blur_active_element;
use crate::session::audio::AudioPreset;
use crate::session::video_mode::VideoMode;

#[component]
pub fn TransmissionMenu<FV, FA>(
    /// Current video mode; its row highlights the matching option.
    video_mode: RwSignal<VideoMode>,
    /// Invoked with a `VideoMode::value()` when a video option is picked.
    on_video_mode: FV,
    /// Current audio preset.
    audio_preset: RwSignal<AudioPreset>,
    /// Invoked with an `AudioPreset::value()` when an audio option is picked.
    on_audio_preset: FA,
    /// Whether the *current* share actually carries an audio track. False for
    /// a plain browser tab sharing a whole screen or a window — Chrome only
    /// offers "share tab audio" for a shared tab — so the audio-quality and
    /// mute rows, which would control a track that isn't there, stay hidden.
    has_audio: RwSignal<bool>,
    /// Whether the sharer has silenced their own outgoing audio.
    audio_muted: RwSignal<bool>,
) -> impl IntoView
where
    FV: Fn(&'static str) + Clone + 'static,
    FA: Fn(&'static str) + Clone + 'static,
{
    let video_opts = VideoMode::ALL
        .iter()
        .map(|mode| {
            let mode = *mode;
            let on_video_mode = on_video_mode.clone();
            let is_on = move || video_mode.get() == mode;
            view! {
                <button
                    type="button"
                    class="transmission-menu__opt"
                    class:transmission-menu__opt--on=is_on
                    title=mode.hint()
                    on:click=move |_| {
                        on_video_mode(mode.value());
                        blur_active_element();
                    }
                >
                    {mode.label()}
                </button>
            }
        })
        .collect_view();

    let audio_opts = AudioPreset::ALL
        .iter()
        .map(|preset| {
            let preset = *preset;
            let on_audio_preset = on_audio_preset.clone();
            let is_on = move || audio_preset.get() == preset;
            view! {
                <button
                    type="button"
                    class="transmission-menu__opt"
                    class:transmission-menu__opt--on=is_on
                    title=preset.hint()
                    on:click=move |_| {
                        on_audio_preset(preset.value());
                        blur_active_element();
                    }
                >
                    {preset.label()}
                </button>
            }
        })
        .collect_view();

    view! {
        <div class="transmission-menu">
            <button
                type="button"
                class="icon-btn icon-btn--neutral transmission-menu__trigger"
                title="Ajustes da transmissão"
                aria-label="Ajustes da transmissão"
                aria-haspopup="menu"
            >
                {icon_sliders()}
            </button>
            <div class="transmission-menu__popup" role="menu">
                <div class="transmission-menu__group">
                    <span class="transmission-menu__label">"Modo de vídeo"</span>
                    <div class="transmission-menu__opts">{video_opts}</div>
                </div>
                <div class="transmission-menu__group" class:hidden=move || !has_audio.get()>
                    <span class="transmission-menu__label">"Qualidade do áudio"</span>
                    <div class="transmission-menu__opts">{audio_opts}</div>
                </div>
                <div class="transmission-menu__group" class:hidden=move || !has_audio.get()>
                    <button
                        type="button"
                        class="transmission-menu__opt transmission-menu__mute"
                        class:transmission-menu__opt--on=move || audio_muted.get()
                        aria-pressed=move || audio_muted.get().to_string()
                        on:click=move |_| {
                            audio_muted.update(|m| *m = !*m);
                            // Drop focus so the popup (open on `:focus-within`)
                            // closes when the pointer leaves — same as the
                            // video and audio-quality options above.
                            blur_active_element();
                        }
                    >
                        {move || {
                            if audio_muted.get() {
                                icon_volume_off().into_any()
                            } else {
                                icon_volume().into_any()
                            }
                        }}
                        <span>
                            {move || {
                                if audio_muted.get() { "Áudio mudo" } else { "Áudio ligado" }
                            }}
                        </span>
                    </button>
                </div>
            </div>
        </div>
    }
}
