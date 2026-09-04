//! Driving one `RtcPeerConnection` through offer / answer / ICE, with the
//! music-grade Opus and start-bitrate SDP tuning applied on both
//! descriptions (see `screen_share_domain::sdp`).

use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{RtcIceCandidateInit, RtcPeerConnection, RtcSdpType, RtcSessionDescriptionInit};

pub async fn create_offer(pc: &RtcPeerConnection) -> Result<String, JsValue> {
    let offer = JsFuture::from(pc.create_offer()).await?;
    let sdp = js_sys::Reflect::get(&offer, &JsValue::from_str("sdp"))?
        .as_string()
        .ok_or_else(|| JsValue::from_str("offer has no sdp"))?;
    // Negotiate music-grade stereo Opus — the browser otherwise settles on
    // a mono voice profile, which is wrong for shared system audio. The
    // same edited SDP is set locally and sent, so both sides agree.
    let sdp = screen_share_domain::sdp::tune_opus_for_music(&sdp);
    // Carry the `x-google-*` bitrate hints in the offer too. Chrome reads
    // them for the sending direction from the *remote* description
    // (re-applied in `accept_answer`), not this one, so this is belt-and-
    // braces — it matters only if the far end ever sends video back — but
    // keeping both descriptions symmetric avoids a confusing diff.
    let sdp = screen_share_domain::sdp::tune_video_start_bitrate(&sdp);

    let desc = RtcSessionDescriptionInit::new(RtcSdpType::Offer);
    desc.set_sdp(&sdp);
    JsFuture::from(pc.set_local_description(&desc)).await?;

    Ok(sdp)
}

pub async fn create_answer(pc: &RtcPeerConnection, offer_sdp: &str) -> Result<String, JsValue> {
    let remote_desc = RtcSessionDescriptionInit::new(RtcSdpType::Offer);
    remote_desc.set_sdp(offer_sdp);
    JsFuture::from(pc.set_remote_description(&remote_desc)).await?;

    let answer = JsFuture::from(pc.create_answer()).await?;
    let sdp = js_sys::Reflect::get(&answer, &JsValue::from_str("sdp"))?
        .as_string()
        .ok_or_else(|| JsValue::from_str("answer has no sdp"))?;
    // Match the offerer's Opus tuning so the negotiated direction is stereo
    // both ways (see `create_offer`).
    let sdp = screen_share_domain::sdp::tune_opus_for_music(&sdp);

    let local_desc = RtcSessionDescriptionInit::new(RtcSdpType::Answer);
    local_desc.set_sdp(&sdp);
    JsFuture::from(pc.set_local_description(&local_desc)).await?;

    Ok(sdp)
}

pub async fn accept_answer(pc: &RtcPeerConnection, answer_sdp: &str) -> Result<(), JsValue> {
    // The sharer is the offerer, so this answer becomes the *remote*
    // description its own encoder reads codec parameters from. Chrome honours
    // `x-google-start-bitrate` (and the Opus fmtp tuning) only from the
    // remote description on the sending side, and strips both from the answer
    // it generates — re-assert them here, before `setRemoteDescription`, or
    // the video encoder opens at Chrome's ~300 kbit/s default and crawls up
    // for 10-30 s while `QualityLevel::Auto` sits pinned at `High` waiting
    // for a link it never actually tried to fill. Both passes are idempotent,
    // so a Chrome build that already echoed the keys back is unaffected.
    let sdp = screen_share_domain::sdp::tune_opus_for_music(answer_sdp);
    let sdp = screen_share_domain::sdp::tune_video_start_bitrate(&sdp);

    let remote_desc = RtcSessionDescriptionInit::new(RtcSdpType::Answer);
    remote_desc.set_sdp(&sdp);
    JsFuture::from(pc.set_remote_description(&remote_desc)).await?;
    Ok(())
}

pub fn add_ice_candidate(
    pc: &RtcPeerConnection,
    candidate: &str,
    sdp_mid: Option<String>,
    sdp_m_line_index: Option<u16>,
) {
    let init = RtcIceCandidateInit::new(candidate);
    init.set_sdp_mid(sdp_mid.as_deref());
    init.set_sdp_m_line_index(sdp_m_line_index);
    let _ = pc.add_ice_candidate_with_opt_rtc_ice_candidate_init(Some(&init));
}

#[cfg(all(test, target_arch = "wasm32", feature = "hydrate"))]
#[path = "connection_wasm_tests.rs"]
mod wasm_tests;
