//! A self-test for a share's audio. When a member starts sharing with
//! system audio, silence can mean several things they'd want to know about:
//! the capture failed outright, the output device is muted, or the app they
//! picked isn't actually playing. The browser reports none of this — a
//! dead-silent track looks identical to a working one — so we tap the
//! outgoing track for a couple of seconds, measure it, and surface a
//! warning if nothing came through.
//!
//! The measurement (`rms`) and the verdict (`classify`) are plain functions
//! with no browser in the loop; `probe_share_audio` is the `hydrate`-only
//! wiring that feeds them real samples.

/// Below this RMS a block of audio is inaudible — roughly -80 dBFS, under
/// the noise floor of a "silent" capture but far below any real playback.
pub const SILENCE_RMS: f32 = 1.0e-4;

/// How many measurement blocks the probe takes before it's willing to call
/// a track silent — enough to span short gaps between sounds without making
/// the sharer wait long for the verdict.
#[cfg(any(test, feature = "hydrate"))]
pub const PROBE_BLOCKS: u32 = 12;

/// Root-mean-square amplitude of a block of `[-1.0, 1.0]` PCM samples. `0.0`
/// for an empty block.
pub fn rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum_sq: f64 = samples.iter().map(|&s| (s as f64) * (s as f64)).sum();
    ((sum_sq / samples.len() as f64).sqrt()) as f32
}

/// Whether a block's RMS is below the audible threshold.
pub fn is_effectively_silent(block_rms: f32) -> bool {
    block_rms < SILENCE_RMS
}

/// What the self-test concluded about a share's audio.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioHealth {
    /// Audio wasn't part of this share (a plain browser tab, or the sharer
    /// didn't enable system audio) — nothing to check or warn about.
    NotShared,
    /// A track is present and carried sound during the probe.
    Ok,
    /// Audio was meant to be shared but no track was captured at all.
    CaptureFailed,
    /// A track is present but stayed silent for the whole probe — a muted
    /// output device, or a source that isn't playing.
    Silent,
}

impl AudioHealth {
    /// The message to show the sharer, or `None` when nothing is wrong.
    pub fn warning(self) -> Option<&'static str> {
        match self {
            AudioHealth::NotShared | AudioHealth::Ok => None,
            AudioHealth::CaptureFailed => {
                Some("Compartilhando sem áudio: a captura de áudio do sistema falhou.")
            }
            AudioHealth::Silent => {
                Some("Compartilhando sem som: nenhum áudio foi detectado (verifique o volume ou o app escolhido).")
            }
        }
    }
}

/// Turns the raw observations into a verdict.
///
/// - `audio_expected`: did this share ask for system audio at all?
/// - `audio_track_present`: did the captured stream end up with an audio track?
/// - `heard_any_sound`: did any probe block clear [`SILENCE_RMS`]?
pub fn classify(
    audio_expected: bool,
    audio_track_present: bool,
    heard_any_sound: bool,
) -> AudioHealth {
    if !audio_expected {
        return AudioHealth::NotShared;
    }
    if !audio_track_present {
        return AudioHealth::CaptureFailed;
    }
    if heard_any_sound {
        AudioHealth::Ok
    } else {
        AudioHealth::Silent
    }
}

/// Delay between probe blocks. `PROBE_BLOCKS` of these sets the total window
/// (~1.8 s) the sharer waits for a verdict.
#[cfg(feature = "hydrate")]
const PROBE_BLOCK_INTERVAL_MS: i32 = 150;

/// FFT size for the analyser; also the number of time-domain samples each
/// `get_float_time_domain_data` call returns. One block at 48 kHz is ~21 ms
/// of audio — long enough for a representative RMS.
#[cfg(feature = "hydrate")]
const PROBE_FFT_SIZE: u32 = 1024;

/// Taps `stream`'s first audio track for `PROBE_BLOCKS` short blocks and
/// returns a verdict. Does no work (returns immediately) when there's no
/// audio track — `classify` still decides whether that's a problem based on
/// `audio_expected`. Any Web Audio failure is treated as "couldn't hear
/// anything" rather than propagated: the probe is advisory, it must never
/// break sharing.
#[cfg(feature = "hydrate")]
pub(crate) async fn probe_share_audio(
    stream: &web_sys::MediaStream,
    audio_expected: bool,
) -> AudioHealth {
    use wasm_bindgen::JsCast;

    let has_audio_track = stream
        .get_tracks()
        .iter()
        .filter_map(|t| t.dyn_into::<web_sys::MediaStreamTrack>().ok())
        .any(|t| t.kind() == "audio");

    if !has_audio_track {
        return classify(audio_expected, false, false);
    }

    let heard = listen_for_sound(stream).await.unwrap_or(false);
    classify(audio_expected, true, heard)
}

/// The Web Audio half of the probe, split out so `probe_share_audio` stays
/// readable. `Err` if the graph couldn't be built at all; `Ok(true)` as
/// soon as one block clears the silence threshold.
#[cfg(feature = "hydrate")]
async fn listen_for_sound(stream: &web_sys::MediaStream) -> Result<bool, wasm_bindgen::JsValue> {
    use wasm_bindgen_futures::JsFuture;

    let ctx = web_sys::AudioContext::new()?;
    let source = ctx.create_media_stream_source(stream)?;
    let analyser = ctx.create_analyser()?;
    analyser.set_fft_size(PROBE_FFT_SIZE);
    source.connect_with_audio_node(&analyser)?;

    let mut block = vec![0.0f32; analyser.fft_size() as usize];
    let mut heard = false;
    for _ in 0..PROBE_BLOCKS {
        analyser.get_float_time_domain_data(&mut block);
        if !is_effectively_silent(rms(&block)) {
            heard = true;
            break;
        }
        sleep(PROBE_BLOCK_INTERVAL_MS).await;
    }

    // Best-effort teardown — a leaked context would keep the tab's audio
    // graph alive, but a failing `close()` isn't worth surfacing.
    if let Ok(promise) = ctx.close() {
        let _ = JsFuture::from(promise).await;
    }
    Ok(heard)
}

#[cfg(feature = "hydrate")]
async fn sleep(ms: i32) {
    use wasm_bindgen_futures::JsFuture;

    let promise = js_sys::Promise::new(&mut |resolve, _reject| {
        if let Some(window) = web_sys::window() {
            let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, ms);
        }
    });
    let _ = JsFuture::from(promise).await;
}

#[cfg(test)]
#[path = "audio_health_tests.rs"]
mod tests;

#[cfg(all(test, target_arch = "wasm32", feature = "hydrate"))]
#[path = "audio_health_wasm_tests.rs"]
mod wasm_tests;
