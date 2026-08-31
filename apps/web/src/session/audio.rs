//! Sharer-side audio quality: one preset, chosen by whoever is sharing,
//! that pins the outgoing Opus send rate. Audio has no simulcast — there's
//! a single encoding every viewer receives — so unlike video quality this
//! is the sharer's call, not a per-viewer one. Switching is live: it only
//! moves `RTCRtpSender.setParameters().maxBitrate`, which needs no
//! renegotiation, so viewers hear the change without a gap.
//!
//! The SDP negotiation (`session::sdp`) sets the ceiling once at connect
//! time; this only picks where under that ceiling to sit.

use crate::session::sdp::OPUS_MAX_AVERAGE_BITRATE_BPS;

/// Voice: enough for speech, DTX stays off but the rate is low. Screen
/// shares are usually paired with a separate voice call, so this is the
/// "I only care about the occasional beep" setting.
const VOICE_BITRATE_BPS: u32 = 40_000;
/// Balanced (default): clean stereo for music and game audio without
/// spending the whole ceiling.
const BALANCED_BITRATE_BPS: u32 = 128_000;

/// The sharer's outgoing audio quality. `Balanced` by default — good stereo
/// without reserving the entire negotiated ceiling for one member's audio.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AudioPreset {
    Voice,
    #[default]
    Balanced,
    Music,
}

impl AudioPreset {
    /// Every preset, in the order the segmented control lists them.
    pub const ALL: [AudioPreset; 3] = [
        AudioPreset::Voice,
        AudioPreset::Balanced,
        AudioPreset::Music,
    ];

    /// The `maxBitrate` (bits per second) this preset pins the audio sender
    /// to. Never above [`OPUS_MAX_AVERAGE_BITRATE_BPS`] — the SDP wouldn't
    /// honour a higher value anyway.
    pub fn bitrate_bps(self) -> u32 {
        match self {
            AudioPreset::Voice => VOICE_BITRATE_BPS,
            AudioPreset::Balanced => BALANCED_BITRATE_BPS,
            AudioPreset::Music => OPUS_MAX_AVERAGE_BITRATE_BPS,
        }
    }

    /// Stable identity used by the segmented control — never shown.
    pub fn value(self) -> &'static str {
        match self {
            AudioPreset::Voice => "voice",
            AudioPreset::Balanced => "balanced",
            AudioPreset::Music => "music",
        }
    }

    pub fn from_value(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|preset| preset.value() == value)
    }

    /// Button label in the control.
    pub fn label(self) -> &'static str {
        match self {
            AudioPreset::Voice => "Voz",
            AudioPreset::Balanced => "Balanceado",
            AudioPreset::Music => "Música",
        }
    }

    /// Tooltip explaining what the preset is for.
    pub fn hint(self) -> &'static str {
        match self {
            AudioPreset::Voice => "Fala, screencasts — bitrate baixo",
            AudioPreset::Balanced => "Uso geral — estéreo limpo",
            AudioPreset::Music => "Spotify, jogos, filme — estéreo cheio",
        }
    }
}

/// Pins the audio `RTCRtpSender` on `pc` to `preset`'s bitrate. A no-op if
/// there's no audio track on this connection (a share without system
/// audio, or the sender not established yet) — same tolerance as
/// `quality::apply_tier` for the video side.
#[cfg(feature = "hydrate")]
pub(crate) async fn apply_audio_preset(
    pc: &web_sys::RtcPeerConnection,
    preset: AudioPreset,
) -> Result<(), wasm_bindgen::JsValue> {
    use wasm_bindgen::JsCast;
    use wasm_bindgen_futures::JsFuture;

    let sender = pc.get_senders().iter().find_map(|entry| {
        let sender: web_sys::RtcRtpSender = entry.unchecked_into();
        let is_audio = sender.track().is_some_and(|track| track.kind() == "audio");
        is_audio.then_some(sender)
    });
    let Some(sender) = sender else { return Ok(()) };

    let params = sender.get_parameters();
    let encodings = params.get_encodings().unwrap_or_else(js_sys::Array::new);
    if encodings.length() == 0 {
        let encoding = web_sys::RtcRtpEncodingParameters::new();
        encoding.set_max_bitrate(preset.bitrate_bps());
        encodings.push(&encoding);
    } else {
        for entry in encodings.iter() {
            let encoding: web_sys::RtcRtpEncodingParameters = entry.unchecked_into();
            encoding.set_max_bitrate(preset.bitrate_bps());
        }
    }
    params.set_encodings(&encodings);

    JsFuture::from(sender.set_parameters_with_parameters(&params)).await?;
    Ok(())
}

/// Applies `preset` to every connection this member is currently sending a
/// share to — used when the sharer changes the preset mid-session.
#[cfg(feature = "hydrate")]
pub(crate) async fn apply_audio_preset_to_all(
    conn: &crate::session::RoomSession,
    preset: AudioPreset,
) {
    let peers: Vec<web_sys::RtcPeerConnection> = conn.outgoing.borrow().values().cloned().collect();
    for pc in &peers {
        let _ = apply_audio_preset(pc, preset).await;
    }
}

#[cfg(not(feature = "hydrate"))]
pub(crate) fn set_audio_preset_handler(
    _conn: crate::session::RoomSession,
    _audio_preset: leptos::prelude::RwSignal<AudioPreset>,
) -> impl Fn(&'static str) + Clone + 'static {
    move |_| {}
}

/// Click handler for the audio-quality segmented control: parse the chosen
/// value, update the signal, and re-apply live to every viewer connection.
#[cfg(feature = "hydrate")]
pub(crate) fn set_audio_preset_handler(
    conn: crate::session::RoomSession,
    audio_preset: leptos::prelude::RwSignal<AudioPreset>,
) -> impl Fn(&'static str) + Clone + 'static {
    use leptos::prelude::*;
    use leptos::task::spawn_local;

    move |value| {
        let Some(preset) = AudioPreset::from_value(value) else {
            return;
        };
        audio_preset.set(preset);
        let conn = conn.clone();
        spawn_local(async move {
            apply_audio_preset_to_all(&conn, preset).await;
        });
    }
}

/// Mutes or unmutes the outgoing shared audio by toggling `enabled` on the
/// local stream's audio track(s). The track stays published — viewers just
/// receive silence — so unmuting is instant with no renegotiation. A no-op
/// for a share with no audio (a plain browser tab).
// Only called from `hydrate`-gated code (the mute effect in `RoomPage` and
// `media::switch_source_handler`), so an `ssr`-only build sees no callers.
#[cfg(not(feature = "hydrate"))]
#[allow(dead_code)]
pub(crate) fn set_shared_audio_muted(_conn: &crate::session::RoomSession, _muted: bool) {}

#[cfg(feature = "hydrate")]
pub(crate) fn set_shared_audio_muted(conn: &crate::session::RoomSession, muted: bool) {
    use wasm_bindgen::JsCast;

    let Some(stream) = conn.local_stream.borrow().as_ref().cloned() else {
        return;
    };
    for track in stream.get_tracks().iter() {
        let track: web_sys::MediaStreamTrack = track.unchecked_into();
        if track.kind() == "audio" {
            track.set_enabled(!muted);
        }
    }
}

#[cfg(test)]
#[path = "audio_tests.rs"]
mod tests;

#[cfg(all(test, target_arch = "wasm32", feature = "hydrate"))]
#[path = "audio_wasm_tests.rs"]
mod wasm_tests;
