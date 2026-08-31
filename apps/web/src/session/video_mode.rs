//! Sharer-side video mode: whether the encoder should protect spatial
//! detail or motion when the connection is squeezed. Like [`AudioPreset`]
//! it's the sharer's single choice for the whole room (there's one encoding
//! per viewer connection, tuned the same way), and switching is live — it
//! only rewrites `contentHint` on the track and `degradationPreference` on
//! each sender, neither of which needs renegotiation.
//!
//! [`AudioPreset`]: crate::session::audio::AudioPreset

/// What the encoder should sacrifice first under bandwidth or CPU pressure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VideoMode {
    /// Code, documents, design work: keep the pixels sharp, drop frames
    /// first.
    Detail,
    /// Games and video: keep motion smooth, drop resolution first. This is
    /// the default — a stuttering game or video reads as broken, whereas a
    /// briefly softer document is still perfectly usable, so it's the safer
    /// mode to start in.
    #[default]
    Motion,
}

impl VideoMode {
    /// Every mode, in the order the picker lists them.
    pub const ALL: [VideoMode; 2] = [VideoMode::Detail, VideoMode::Motion];

    /// Stable identity used by the picker and by any persisted
    /// preference — never shown to the user.
    pub fn value(self) -> &'static str {
        match self {
            VideoMode::Detail => "detail",
            VideoMode::Motion => "motion",
        }
    }

    pub fn from_value(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|mode| mode.value() == value)
    }

    /// Button label in the control — names the situation, not the encoder
    /// trade-off, so it means something to someone who isn't thinking about
    /// resolution vs. frame rate.
    pub fn label(self) -> &'static str {
        match self {
            VideoMode::Detail => "Textos e código",
            VideoMode::Motion => "Vídeo e jogos",
        }
    }

    /// Tooltip spelling out what each mode is good for and what it trades.
    pub fn hint(self) -> &'static str {
        match self {
            VideoMode::Detail => {
                "Para ler documentos, código, planilhas — mantém a imagem nítida quando a internet aperta"
            }
            VideoMode::Motion => {
                "Para vídeo, jogos, animação — mantém o movimento fluido quando a internet aperta"
            }
        }
    }

    /// The `MediaStreamTrack.contentHint` this mode maps to.
    pub fn content_hint(self) -> &'static str {
        match self {
            VideoMode::Detail => "detail",
            VideoMode::Motion => "motion",
        }
    }
}

/// The `degradationPreference` string this mode maps to. In the current
/// WebRTC spec this is a top-level member of `RTCRtpSendParameters` (not
/// per-encoding, where older drafts — and web-sys 0.3 — put it), so it's
/// set via `Reflect` on the parameters object.
#[cfg(feature = "hydrate")]
fn degradation_preference(mode: VideoMode) -> &'static str {
    match mode {
        VideoMode::Detail => "maintain-resolution",
        VideoMode::Motion => "maintain-framerate",
    }
}

/// Applies `mode` to one connection's outgoing video: the sender's
/// `degradationPreference` and its track's `contentHint`. A no-op if
/// there's no video sender yet — same tolerance as `quality::apply_tier`.
#[cfg(feature = "hydrate")]
pub(crate) async fn apply_video_mode(
    pc: &web_sys::RtcPeerConnection,
    mode: VideoMode,
) -> Result<(), wasm_bindgen::JsValue> {
    use wasm_bindgen::{JsCast, JsValue};
    use wasm_bindgen_futures::JsFuture;

    let sender = pc.get_senders().iter().find_map(|entry| {
        let sender: web_sys::RtcRtpSender = entry.unchecked_into();
        let is_video = sender.track().is_some_and(|track| track.kind() == "video");
        is_video.then_some(sender)
    });
    let Some(sender) = sender else { return Ok(()) };

    if let Some(track) = sender.track() {
        let _ = js_sys::Reflect::set(
            &track,
            &JsValue::from_str("contentHint"),
            &JsValue::from_str(mode.content_hint()),
        );
    }

    let params = sender.get_parameters();
    let _ = js_sys::Reflect::set(
        &params,
        &JsValue::from_str("degradationPreference"),
        &JsValue::from_str(degradation_preference(mode)),
    );
    JsFuture::from(sender.set_parameters_with_parameters(&params)).await?;
    Ok(())
}

/// Applies `mode` to every connection this member is sending a share to,
/// plus the local preview stream's track — used when the sharer switches
/// mode mid-session.
#[cfg(feature = "hydrate")]
pub(crate) async fn apply_video_mode_to_all(conn: &crate::session::RoomSession, mode: VideoMode) {
    use wasm_bindgen::{JsCast, JsValue};

    if let Some(stream) = conn.local_stream.borrow().as_ref() {
        for track in stream.get_tracks().iter() {
            let track: web_sys::MediaStreamTrack = track.unchecked_into();
            if track.kind() == "video" {
                let _ = js_sys::Reflect::set(
                    &track,
                    &JsValue::from_str("contentHint"),
                    &JsValue::from_str(mode.content_hint()),
                );
            }
        }
    }

    let peers: Vec<web_sys::RtcPeerConnection> = conn.outgoing.borrow().values().cloned().collect();
    for pc in &peers {
        let _ = apply_video_mode(pc, mode).await;
    }
}

#[cfg(not(feature = "hydrate"))]
pub(crate) fn set_video_mode_handler(
    _conn: crate::session::RoomSession,
    _video_mode: leptos::prelude::RwSignal<VideoMode>,
) -> impl Fn(&'static str) + Clone + 'static {
    move |_| {}
}

/// Click handler for the video-mode picker: parse the chosen
/// value, update the signal, and re-apply live to every viewer connection.
#[cfg(feature = "hydrate")]
pub(crate) fn set_video_mode_handler(
    conn: crate::session::RoomSession,
    video_mode: leptos::prelude::RwSignal<VideoMode>,
) -> impl Fn(&'static str) + Clone + 'static {
    use leptos::prelude::*;
    use leptos::task::spawn_local;

    move |value| {
        let Some(mode) = VideoMode::from_value(value) else {
            return;
        };
        video_mode.set(mode);
        let conn = conn.clone();
        spawn_local(async move {
            apply_video_mode_to_all(&conn, mode).await;
        });
    }
}

#[cfg(test)]
#[path = "video_mode_tests.rs"]
mod tests;

#[cfg(all(test, target_arch = "wasm32", feature = "hydrate"))]
#[path = "video_mode_wasm_tests.rs"]
mod wasm_tests;
