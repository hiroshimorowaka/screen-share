use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    ConstrainDomStringParameters, DisplayMediaStreamConstraints, MediaStream,
    MediaStreamConstraints, MediaTrackConstraints, RtcConfiguration, RtcIceCandidateInit,
    RtcIceServer, RtcPeerConnection, RtcSdpType, RtcSessionDescriptionInit,
};

pub fn is_desktop_app() -> bool {
    let Some(window) = web_sys::window() else { return false };
    js_sys::Reflect::has(&window, &JsValue::from_str("desktopAudio")).unwrap_or(false)
}

pub async fn capture_display() -> Result<MediaStream, JsValue> {
    let window = web_sys::window().ok_or_else(|| JsValue::from_str("no window: not running in a browser"))?;
    let media_devices = window.navigator().media_devices()?;

    let constraints = DisplayMediaStreamConstraints::new();
    constraints.set_video_bool(true);

    let promise = media_devices.get_display_media_with_constraints(&constraints)?;
    let stream = JsFuture::from(promise).await?;
    let video_stream = stream.dyn_into::<MediaStream>()?;

    // Whether to also attach audio was already decided inside the
    // desktop app's own share picker, before this call was even made —
    // this just tries, and a missing device means audio wasn't
    // requested (not an error) rather than something to report.
    match capture_loopback_audio(&media_devices).await {
        Ok(audio_stream) => combine_video_and_audio(&video_stream, &audio_stream),
        Err(_) => Ok(video_stream),
    }
}

pub async fn stop_desktop_audio_loopback() -> Result<(), JsValue> {
    let window = web_sys::window().ok_or_else(|| JsValue::from_str("no window"))?;
    let desktop_audio = js_sys::Reflect::get(&window, &JsValue::from_str("desktopAudio"))?;
    let stop_fn: js_sys::Function =
        js_sys::Reflect::get(&desktop_audio, &JsValue::from_str("stop"))?.dyn_into()?;
    let promise: js_sys::Promise = stop_fn.call0(&desktop_audio)?.dyn_into()?;
    JsFuture::from(promise).await?;
    Ok(())
}

async fn capture_loopback_audio(media_devices: &web_sys::MediaDevices) -> Result<MediaStream, JsValue> {
    let promise = media_devices.enumerate_devices()?;
    let devices: js_sys::Array = JsFuture::from(promise).await?.dyn_into()?;

    let mut device_id = None;
    for device in devices.iter() {
        let info: web_sys::MediaDeviceInfo = device.dyn_into()?;
        if info.kind() == web_sys::MediaDeviceKind::Audioinput
            && info.label().contains("Screen Share Mix")
        {
            device_id = Some(info.device_id());
        }
    }
    let device_id =
        device_id.ok_or_else(|| JsValue::from_str("Screen Share Mix device not found"))?;

    // `exact` (not `ideal`): if this specific device isn't available for
    // any reason, getUserMedia must reject instead of silently falling
    // back to the system's default microphone.
    let exact = ConstrainDomStringParameters::new();
    exact.set_exact_str(&device_id);
    let track_constraints = MediaTrackConstraints::new();
    track_constraints.set_device_id_constrain_dom_string_parameters(&exact);
    // This is system/music audio, not a voice call — Chromium's default
    // voice-call audio processing (tuned for a mic) is actively harmful
    // here, not just pointless.
    track_constraints.set_echo_cancellation_bool(false);
    track_constraints.set_noise_suppression_bool(false);
    track_constraints.set_auto_gain_control_bool(false);
    let audio_constraints = MediaStreamConstraints::new();
    audio_constraints.set_audio_media_track_constraints(&track_constraints);

    let promise = media_devices.get_user_media_with_constraints(&audio_constraints)?;
    JsFuture::from(promise).await?.dyn_into::<MediaStream>()
}

fn combine_video_and_audio(video: &MediaStream, audio: &MediaStream) -> Result<MediaStream, JsValue> {
    let tracks = js_sys::Array::new();
    for track in video.get_tracks().iter() {
        tracks.push(&track);
    }
    for track in audio.get_tracks().iter() {
        tracks.push(&track);
    }
    MediaStream::new_with_tracks(&tracks)
}

/// A public STUN server, used only so each peer can discover its own
/// public-facing address for the ICE candidates it offers — no media or
/// signaling data ever passes through it.
const STUN_SERVER_URL: &str = "stun:stun.l.google.com:19302";

pub fn new_peer_connection() -> Result<RtcPeerConnection, JsValue> {
    let ice_server = RtcIceServer::new();
    let urls = js_sys::Array::new();
    urls.push(&JsValue::from_str(STUN_SERVER_URL));
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
        .ok_or_else(|| JsValue::from_str("offer has no sdp"))?;

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
