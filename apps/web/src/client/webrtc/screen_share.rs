//! `getDisplayMedia` capture: the constraints, the capture call itself
//! (which stitches in the desktop shell's system audio when a desktop
//! share includes it), and the `getDisplayMedia` support probe.

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::{DisplayMediaStreamConstraints, MediaStream};

use crate::client::desktop_bridge::{
    build_track_from_pcm_bridge, capture_loopback_audio, desktop_audio_loopback_active,
    has_pcm_bridge, is_desktop_app,
};

/// The `getDisplayMedia` constraints for a capture started here.
///
/// `desktop` (the Electron shell) captures audio through its own platform
/// backend and only ever wants video from `getDisplayMedia`. A plain
/// browser tab has no such backend, so it asks for audio here too: Chrome's
/// own picker then offers a "share tab audio" checkbox, and a ticked box
/// puts an audio track on the returned stream (browser capture only carries
/// the audio of a shared *tab*, never a window or the whole system).
fn display_media_constraints(desktop: bool) -> DisplayMediaStreamConstraints {
    let constraints = DisplayMediaStreamConstraints::new();
    constraints.set_video_bool(true);
    if !desktop {
        constraints.set_audio_bool(true);
    }
    constraints
}

pub async fn capture_display() -> Result<MediaStream, JsValue> {
    let window = web_sys::window()
        .ok_or_else(|| JsValue::from_str("no window: not running in a browser"))?;
    let media_devices = window.navigator().media_devices()?;

    let desktop = is_desktop_app();
    let constraints = display_media_constraints(desktop);

    let promise = media_devices.get_display_media_with_constraints(&constraints)?;
    let stream = JsFuture::from(promise).await?;
    let video_stream = stream.dyn_into::<MediaStream>()?;
    // The video track's `contentHint` (and the sender's degradation
    // preference) is owned by `session::video_mode` — applied per viewer
    // connection when it opens and re-applied whenever the sharer changes
    // mode. Nothing to set here at capture time.

    // A plain browser tab's audio, when the sharer opted into it in the
    // picker, is already a track on this stream; the desktop-only bridge
    // paths below don't apply.
    if !desktop {
        return Ok(video_stream);
    }

    // The share picker (Electron side) already decided whether this share
    // includes audio, and started the platform loopback if so. Only probe
    // for the captured audio when it's actually running — otherwise a
    // deliberately audio-less share logs a spurious "device not found"
    // and, worse, `getUserMedia` for a vanished device can be rerouted to
    // the default mic.
    if !desktop_audio_loopback_active().await {
        return Ok(video_stream);
    }

    // The Windows desktop app bridges captured PCM over IPC instead of
    // exposing a capturable device — same intent as the Linux path below,
    // different mechanism. `has_pcm_bridge()` is the one signal that
    // distinguishes them, since it's never true outside the Windows
    // desktop app.
    if has_pcm_bridge() {
        return match build_track_from_pcm_bridge().await {
            Ok(audio_stream) => combine_video_and_audio(&video_stream, &audio_stream),
            Err(err) => {
                web_sys::console::error_2(
                    &JsValue::from_str(
                        "build_track_from_pcm_bridge failed, falling back to video-only:",
                    ),
                    &err,
                );
                Ok(video_stream)
            }
        };
    }

    match capture_loopback_audio(&media_devices).await {
        Ok(audio_stream) => combine_video_and_audio(&video_stream, &audio_stream),
        Err(err) => {
            web_sys::console::error_2(
                &JsValue::from_str("capture_loopback_audio failed, falling back to video-only:"),
                &err,
            );
            Ok(video_stream)
        }
    }
}

fn combine_video_and_audio(
    video: &MediaStream,
    audio: &MediaStream,
) -> Result<MediaStream, JsValue> {
    let tracks = js_sys::Array::new();
    for track in video.get_tracks().iter() {
        tracks.push(&track);
    }
    for track in audio.get_tracks().iter() {
        tracks.push(&track);
    }
    MediaStream::new_with_tracks(&tracks)
}

pub fn is_display_media_supported() -> bool {
    let Some(window) = web_sys::window() else {
        return false;
    };
    let Ok(media_devices) = window.navigator().media_devices() else {
        return false;
    };
    js_sys::Reflect::has(&media_devices, &JsValue::from_str("getDisplayMedia")).unwrap_or(false)
}

#[cfg(all(test, target_arch = "wasm32", feature = "hydrate"))]
#[path = "screen_share_wasm_tests.rs"]
mod wasm_tests;
