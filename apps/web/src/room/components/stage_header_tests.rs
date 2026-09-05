//! `AudioChip`'s red/amber distinction: a capture that failed outright
//! (no track at all) is a hard error, red like every other error status
//! in the app; a track that merely stayed silent so far is still just the
//! amber "!" tooltip, since it might start carrying sound any moment.
//! Split out to keep `stage_header.rs` itself readable (in-crate, not
//! `apps/web/tests/ssr_render.rs`: `AudioChip` isn't part of the crate's
//! public API).

use leptos::prelude::*;

use super::AudioChip;
use crate::room::audio_health::AudioHealth;

fn render(audio_health: AudioHealth, share_has_audio: bool) -> String {
    let owner = Owner::new();
    owner.with(|| {
        let (is_sharing, _) = signal(true);
        let audio_muted = RwSignal::new(false);
        let share_has_audio = RwSignal::new(share_has_audio);
        let audio_health = RwSignal::new(audio_health);
        view! {
            <AudioChip
                is_sharing=is_sharing
                audio_muted=audio_muted
                share_has_audio=share_has_audio
                audio_health=audio_health
                sharing_has_audio=true
            />
        }
        .into_view()
        .to_html()
    })
}

#[test]
fn a_failed_capture_marks_the_chip_with_the_error_modifier() {
    let html = render(AudioHealth::CaptureFailed, false);

    assert!(html.contains("audio-chip--error"), "html was: {html}");
    assert!(html.contains("Áudio desligado"));
}

#[test]
fn a_merely_silent_track_does_not_get_the_error_modifier() {
    // Regression guard: before `AudioHealth` was threaded through, both
    // "capture failed" and "still silent" collapsed into the same amber
    // warning — a real, permanent failure looked identical to a track
    // that just hadn't made noise yet.
    let html = render(AudioHealth::Silent, true);

    assert!(!html.contains("audio-chip--error"), "html was: {html}");
    assert!(html.contains("Áudio ligado"));
}

#[test]
fn healthy_audio_gets_neither_modifier() {
    let html = render(AudioHealth::Ok, true);

    assert!(!html.contains("audio-chip--error"));
    assert!(!html.contains("audio-chip--muted"));
}
