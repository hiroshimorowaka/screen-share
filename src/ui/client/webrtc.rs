use std::cell::Cell;
use std::rc::Rc;

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    AudioData, AudioDataInit, AudioSampleFormat, ConstrainDomStringParameters,
    DisplayMediaStreamConstraints, MediaStream, MediaStreamConstraints, MediaStreamTrackGenerator,
    MediaStreamTrackGeneratorInit, MediaTrackConstraints, RtcConfiguration, RtcIceCandidateInit,
    RtcIceServer, RtcPeerConnection, RtcSdpType, RtcSessionDescriptionInit,
    WritableStreamDefaultWriter,
};

pub fn is_desktop_app() -> bool {
    let Some(window) = web_sys::window() else {
        return false;
    };
    js_sys::Reflect::has(&window, &JsValue::from_str("desktopAudio")).unwrap_or(false)
}

/// Hands the invite link for a just-started share to the desktop shell's
/// `window.desktopShare.linkReady` bridge, so it can copy it to the
/// clipboard on the sharer's behalf. Only the desktop app's preload script
/// ever defines that bridge, so this is a no-op in a plain browser tab —
/// and it has to go through the shell rather than the page's own Clipboard
/// API, since the quick-share flow's window stays hidden throughout,
/// and that API requires document focus.
pub fn notify_desktop_share_ready(link: &str) {
    if !is_desktop_app() {
        return;
    }
    let Some(window) = web_sys::window() else {
        return;
    };
    let Ok(bridge) = js_sys::Reflect::get(&window, &JsValue::from_str("desktopShare")) else {
        return;
    };
    let Ok(link_ready) = js_sys::Reflect::get(&bridge, &JsValue::from_str("linkReady")) else {
        return;
    };
    let Ok(link_ready) = link_ready.dyn_into::<js_sys::Function>() else {
        return;
    };
    let _ = link_ready.call1(&bridge, &JsValue::from_str(link));
}

pub async fn capture_display() -> Result<MediaStream, JsValue> {
    let window = web_sys::window()
        .ok_or_else(|| JsValue::from_str("no window: not running in a browser"))?;
    let media_devices = window.navigator().media_devices()?;

    let constraints = DisplayMediaStreamConstraints::new();
    constraints.set_video_bool(true);

    let promise = media_devices.get_display_media_with_constraints(&constraints)?;
    let stream = JsFuture::from(promise).await?;
    let video_stream = stream.dyn_into::<MediaStream>()?;

    // The Windows desktop app bridges captured PCM over IPC instead of
    // exposing a capturable device — same intent as the Linux path below
    // (audio was already decided inside the share picker; a missing
    // bridge/device just means audio wasn't requested), different
    // mechanism. `has_pcm_bridge()` is the one signal that distinguishes
    // them, since it's never true outside the Windows desktop app.
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

fn has_pcm_bridge() -> bool {
    let Some(window) = web_sys::window() else {
        return false;
    };
    let Ok(desktop_audio) = js_sys::Reflect::get(&window, &JsValue::from_str("desktopAudio"))
    else {
        return false;
    };
    js_sys::Reflect::has(&desktop_audio, &JsValue::from_str("onPcmChunk")).unwrap_or(false)
}

// The mix the native Windows module (`desktop/native/windows-audio`)
// produces and sends over IPC — interleaved stereo f32 PCM, matching
// exactly what its `capture.rs` mixer emits.
const PCM_BRIDGE_CHANNELS: u32 = 2;
const PCM_BRIDGE_SAMPLE_RATE: f32 = 48_000.0;
const PCM_BRIDGE_BYTES_PER_SAMPLE: u32 = 4;

/// Turns the desktop app's `window.desktopAudio.onPcmChunk` PCM stream
/// into a real `MediaStreamTrack` via `MediaStreamTrackGenerator` +
/// `AudioData` — see Task 1 in the Windows audio sharing plan for how
/// this was proven to work (format `'f32'`, a monotonically increasing
/// microsecond timestamp derived from cumulative frames written, no
/// backpressure at the 20ms/960-frame chunk cadence the native mixer
/// uses).
async fn build_track_from_pcm_bridge() -> Result<MediaStream, JsValue> {
    let window = web_sys::window().ok_or_else(|| JsValue::from_str("no window"))?;
    let desktop_audio = js_sys::Reflect::get(&window, &JsValue::from_str("desktopAudio"))?;
    let on_pcm_chunk: js_sys::Function =
        js_sys::Reflect::get(&desktop_audio, &JsValue::from_str("onPcmChunk"))?.dyn_into()?;

    let init = MediaStreamTrackGeneratorInit::new("audio");
    let generator = MediaStreamTrackGenerator::new(&init)?;
    let writer: WritableStreamDefaultWriter = generator.writable().get_writer()?;

    // Cumulative frames written, so each chunk's timestamp is
    // monotonically increasing — matches what Task 1 confirmed actually
    // works. `Cell`, not `AtomicU64`: this closure only ever runs on the
    // single-threaded JS event loop.
    let frames_written = Rc::new(Cell::new(0u64));
    // A per-chunk failure here would otherwise be completely invisible —
    // the closure has no return value the caller ever inspects, unlike
    // the one-time setup above this — so it's logged, but only once per
    // failure kind, since this runs roughly every 20ms and would
    // otherwise flood the console.
    let logged_array_buffer_failure = Rc::new(Cell::new(false));
    let logged_audio_data_failure = Rc::new(Cell::new(false));
    let logged_write_failure = Rc::new(Cell::new(false));

    let on_chunk = Closure::<dyn FnMut(JsValue)>::new(move |chunk: JsValue| {
        let Ok(array_buffer) = chunk.dyn_into::<js_sys::ArrayBuffer>() else {
            if !logged_array_buffer_failure.replace(true) {
                web_sys::console::error_1(&JsValue::from_str(
                    "PCM bridge chunk wasn't an ArrayBuffer; further occurrences won't be logged",
                ));
            }
            return;
        };
        let frames =
            array_buffer.byte_length() / (PCM_BRIDGE_CHANNELS * PCM_BRIDGE_BYTES_PER_SAMPLE);
        if frames == 0 {
            return;
        }

        let data = js_sys::Uint8Array::new(&array_buffer);
        // `AudioDataInit::new(..)` takes an `i32` timestamp, which would
        // overflow (wrapping the timestamp backwards) after ~35 minutes
        // of continuous sharing at this chunk cadence — built by hand
        // instead, the same way `AudioDataInit::new` itself is
        // implemented internally, so `set_timestamp_f64` (the `f64`
        // setter) can be used instead.
        let audio_data_init: AudioDataInit =
            wasm_bindgen::JsCast::unchecked_into(js_sys::Object::new());
        audio_data_init.set_data_u8_array(&data);
        audio_data_init.set_format(AudioSampleFormat::F32);
        audio_data_init.set_number_of_channels(PCM_BRIDGE_CHANNELS);
        audio_data_init.set_number_of_frames(frames);
        audio_data_init.set_sample_rate(PCM_BRIDGE_SAMPLE_RATE);
        let timestamp_us =
            frames_written.get() as f64 / PCM_BRIDGE_SAMPLE_RATE as f64 * 1_000_000.0;
        audio_data_init.set_timestamp_f64(timestamp_us);
        frames_written.set(frames_written.get() + frames as u64);

        let Ok(audio_data) = AudioData::new(&audio_data_init) else {
            if !logged_audio_data_failure.replace(true) {
                web_sys::console::error_1(&JsValue::from_str(
                    "AudioData::new failed for a PCM bridge chunk; further occurrences won't be logged",
                ));
            }
            return;
        };
        // Should only ever reject once the generator's track has ended
        // (e.g. after a later share was stopped); a rejection from the
        // very first chunks would point at something more fundamental,
        // so the first one (only) is logged rather than blanket-swallowed.
        let write_promise = writer.write_with_chunk(&audio_data.into());
        let logged_write_failure = logged_write_failure.clone();
        wasm_bindgen_futures::spawn_local(async move {
            if let Err(err) = JsFuture::from(write_promise).await {
                if !logged_write_failure.replace(true) {
                    web_sys::console::error_2(
                        &JsValue::from_str(
                            "writer.write_with_chunk rejected for a PCM bridge chunk; further occurrences won't be logged:",
                        ),
                        &err,
                    );
                }
            }
        });
    });

    on_pcm_chunk.call1(&desktop_audio, on_chunk.as_ref().unchecked_ref())?;
    // Leaked deliberately — this closure, and the writer it holds by
    // move, need to outlive this function call for the rest of the
    // share. Standard wasm-bindgen practice for a listener with no
    // natural single owner to drop it later.
    on_chunk.forget();

    let tracks = js_sys::Array::new();
    tracks.push(generator.as_ref());
    MediaStream::new_with_tracks(&tracks)
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

async fn capture_loopback_audio(
    media_devices: &web_sys::MediaDevices,
) -> Result<MediaStream, JsValue> {
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
    let Some(window) = web_sys::window() else {
        return false;
    };
    let Ok(media_devices) = window.navigator().media_devices() else {
        return false;
    };
    js_sys::Reflect::has(&media_devices, &JsValue::from_str("getDisplayMedia")).unwrap_or(false)
}
