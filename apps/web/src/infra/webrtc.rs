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

/// Tells the desktop shell's `window.desktopShare.memberJoined` bridge that
/// someone just joined the room, so it can raise a native OS notification —
/// the room page's window stays hidden/backgrounded for most of a desktop
/// session, so an in-page toast would go unseen. No-op in a plain browser
/// tab, same as `notify_desktop_share_ready`.
pub fn notify_desktop_member_joined(nick: &str) {
    if !is_desktop_app() {
        return;
    }
    let Some(window) = web_sys::window() else {
        return;
    };
    let Ok(bridge) = js_sys::Reflect::get(&window, &JsValue::from_str("desktopShare")) else {
        return;
    };
    let Ok(member_joined) = js_sys::Reflect::get(&bridge, &JsValue::from_str("memberJoined"))
    else {
        return;
    };
    let Ok(member_joined) = member_joined.dyn_into::<js_sys::Function>() else {
        return;
    };
    let _ = member_joined.call1(&bridge, &JsValue::from_str(nick));
}

/// Tells the desktop shell's `window.desktopShare.sharingChanged` bridge
/// whether this member is currently sharing, so the tray icon can switch
/// between its idle (green) and live (red) state. No-op in a plain browser
/// tab.
pub fn notify_desktop_sharing_changed(is_sharing: bool) {
    if !is_desktop_app() {
        return;
    }
    let Some(window) = web_sys::window() else {
        return;
    };
    let Ok(bridge) = js_sys::Reflect::get(&window, &JsValue::from_str("desktopShare")) else {
        return;
    };
    let Ok(sharing_changed) = js_sys::Reflect::get(&bridge, &JsValue::from_str("sharingChanged"))
    else {
        return;
    };
    let Ok(sharing_changed) = sharing_changed.dyn_into::<js_sys::Function>() else {
        return;
    };
    let _ = sharing_changed.call1(&bridge, &JsValue::from_bool(is_sharing));
}

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

/// Whether the Electron shell currently has an audio loopback running for
/// this share — i.e. the sharer ticked "compartilhar áudio" in the
/// picker. `false` (its own default) on any error or outside the desktop
/// app.
async fn desktop_audio_loopback_active() -> bool {
    let Some(window) = web_sys::window() else {
        return false;
    };
    let Ok(desktop_audio) = js_sys::Reflect::get(&window, &JsValue::from_str("desktopAudio"))
    else {
        return false;
    };
    let Ok(active_fn) = js_sys::Reflect::get(&desktop_audio, &JsValue::from_str("active")) else {
        return false;
    };
    let Ok(active_fn) = active_fn.dyn_into::<js_sys::Function>() else {
        return false;
    };
    let Ok(result) = active_fn.call0(&desktop_audio) else {
        return false;
    };
    let Ok(promise) = result.dyn_into::<js_sys::Promise>() else {
        return false;
    };
    JsFuture::from(promise)
        .await
        .map(|value| value.is_truthy())
        .unwrap_or(false)
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

/// `turn` is `None` on a deployment with no TURN server configured (or
/// briefly, before the join snapshot carrying it has arrived) — the
/// connection still works for peers that don't need a relay, just without
/// a fallback for the ones that do.
pub fn new_peer_connection(
    turn: Option<&screen_share_protocol::TurnCredentials>,
) -> Result<RtcPeerConnection, JsValue> {
    let stun_server = RtcIceServer::new();
    let stun_urls = js_sys::Array::new();
    stun_urls.push(&JsValue::from_str(STUN_SERVER_URL));
    stun_server.set_urls(&JsValue::from(stun_urls));

    let servers = js_sys::Array::new();
    servers.push(&stun_server);

    if let Some(turn) = turn {
        let turn_server = RtcIceServer::new();
        let turn_urls = js_sys::Array::new();
        for url in &turn.urls {
            turn_urls.push(&JsValue::from_str(url));
        }
        turn_server.set_urls(&JsValue::from(turn_urls));
        turn_server.set_username(&turn.username);
        turn_server.set_credential(&turn.password);
        servers.push(&turn_server);
    }

    let config = RtcConfiguration::new();
    config.set_ice_servers(&JsValue::from(servers));

    RtcPeerConnection::new_with_configuration(&config)
}

/// Negotiates a `sendonly` audio m-line on `pc` up front, with no track,
/// tied to `stream` (the share's video stream).
///
/// Used when a share starts without audio. A later "trocar fonte" can add
/// audio (a shared tab with sound, or the desktop system-audio loopback),
/// and `session::media::replace_outgoing_tracks` swaps it in with
/// `RTCRtpSender.replaceTrack` — which needs no renegotiation, but only
/// reaches an m-line that already existed when the connection was
/// answered. This signaling path never re-offers an open viewer
/// connection, so without this reservation a viewer already watching a
/// silent share would stay silent after the switch until they re-watched.
/// Binding it to `stream` makes the viewer group the eventual audio track
/// with the video it is already playing, so `ontrack` doesn't need a
/// second stream to attach.
pub fn reserve_audio_mline(pc: &RtcPeerConnection, stream: &MediaStream) {
    let streams = js_sys::Array::new();
    streams.push(stream);
    let init = web_sys::RtcRtpTransceiverInit::new();
    init.set_direction(web_sys::RtcRtpTransceiverDirection::Sendonly);
    init.set_streams(&streams);
    pc.add_transceiver_with_str_and_init("audio", &init);
}

pub async fn create_offer(pc: &RtcPeerConnection) -> Result<String, JsValue> {
    let offer = JsFuture::from(pc.create_offer()).await?;
    let sdp = js_sys::Reflect::get(&offer, &JsValue::from_str("sdp"))?
        .as_string()
        .ok_or_else(|| JsValue::from_str("offer has no sdp"))?;
    // Negotiate music-grade stereo Opus — the browser otherwise settles on
    // a mono voice profile, which is wrong for shared system audio. The
    // same edited SDP is set locally and sent, so both sides agree.
    let sdp = crate::session::sdp::tune_opus_for_music(&sdp);
    // Carry the `x-google-*` bitrate hints in the offer too. Chrome reads
    // them for the sending direction from the *remote* description
    // (re-applied in `accept_answer`), not this one, so this is belt-and-
    // braces — it matters only if the far end ever sends video back — but
    // keeping both descriptions symmetric avoids a confusing diff.
    let sdp = crate::session::sdp::tune_video_start_bitrate(&sdp);

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
    let sdp = crate::session::sdp::tune_opus_for_music(&sdp);

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
    let sdp = crate::session::sdp::tune_opus_for_music(answer_sdp);
    let sdp = crate::session::sdp::tune_video_start_bitrate(&sdp);

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
#[path = "webrtc_wasm_tests.rs"]
mod wasm_tests;
