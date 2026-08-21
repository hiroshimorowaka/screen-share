use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    DisplayMediaStreamConstraints, MediaStream, RtcConfiguration, RtcIceCandidateInit,
    RtcIceServer, RtcPeerConnection, RtcSdpType, RtcSessionDescriptionInit,
};

pub async fn capture_display() -> Result<MediaStream, JsValue> {
    let window = web_sys::window().ok_or_else(|| JsValue::from_str("sem window"))?;
    let media_devices = window.navigator().media_devices()?;

    let constraints = DisplayMediaStreamConstraints::new();
    constraints.set_video_bool(true);

    let promise = media_devices.get_display_media_with_constraints(&constraints)?;
    let stream = JsFuture::from(promise).await?;
    stream.dyn_into::<MediaStream>()
}

pub fn new_peer_connection() -> Result<RtcPeerConnection, JsValue> {
    let ice_server = RtcIceServer::new();
    let urls = js_sys::Array::new();
    urls.push(&JsValue::from_str("stun:stun.l.google.com:19302"));
    ice_server.set_urls(&JsValue::from(urls));

    let servers = js_sys::Array::new();
    servers.push(&ice_server);

    let config = RtcConfiguration::new();
    config.set_ice_servers(&JsValue::from(servers));

    RtcPeerConnection::new_with_configuration(&config)
}

pub async fn create_offer(pc: &RtcPeerConnection) -> Result<String, JsValue> {
    let offer = JsFuture::from(pc.create_offer()).await?;
    let sdp = js_sys::Reflect::get(&offer, &JsValue::from_str("sdp"))?
        .as_string()
        .ok_or_else(|| JsValue::from_str("offer sem sdp"))?;

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
        .ok_or_else(|| JsValue::from_str("answer sem sdp"))?;

    let local_desc = RtcSessionDescriptionInit::new(RtcSdpType::Answer);
    local_desc.set_sdp(&sdp);
    JsFuture::from(pc.set_local_description(&local_desc)).await?;

    Ok(sdp)
}

pub async fn accept_answer(pc: &RtcPeerConnection, answer_sdp: &str) -> Result<(), JsValue> {
    let remote_desc = RtcSessionDescriptionInit::new(RtcSdpType::Answer);
    remote_desc.set_sdp(answer_sdp);
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

pub fn is_display_media_supported() -> bool {
    let Some(window) = web_sys::window() else { return false };
    let Ok(media_devices) = window.navigator().media_devices() else { return false };
    js_sys::Reflect::has(&media_devices, &JsValue::from_str("getDisplayMedia")).unwrap_or(false)
}
