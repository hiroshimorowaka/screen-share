//! Per-viewer video quality: the sharer-side `getStats()`/`setParameters()`
//! mechanics that feed and apply the bandwidth-adaptive hysteresis behind
//! `QualityLevel::Auto`, plus the viewer-side click handler that sends a
//! chosen quality. The pure state machine and the encoding presets live
//! in `screen_share_domain::quality`.

#[cfg(feature = "hydrate")]
use screen_share_domain::quality::{preset_for, EncodingPreset, RawReading};
#[cfg(feature = "hydrate")]
pub(crate) use screen_share_domain::quality::{tier_for, AdaptiveQuality, InitialTier, Tier};

/// How often the sharer polls `getStats()` for a viewer on `Auto` — frequent
/// enough that a real problem gets caught within a few polls (see the
/// hysteresis constants above), infrequent enough that it isn't itself a
/// meaningful load.
#[cfg(feature = "hydrate")]
const AUTO_POLL_INTERVAL_MS: i32 = 3_000;

/// A running Auto-quality poll: its `setInterval` id and the tick closure
/// it drives. Held in `RoomSession::quality_auto_intervals` so
/// [`stop_auto_polling`] both `clearInterval`s and drops the closure —
/// otherwise the closure's captured `RoomSession` clone keeps the whole
/// session graph alive after the poll is gone.
#[cfg(feature = "hydrate")]
pub(crate) struct AutoPoll {
    interval_id: i32,
    _keep_alive: wasm_bindgen::prelude::Closure<dyn FnMut()>,
}

#[cfg(all(feature = "hydrate", test))]
impl AutoPoll {
    /// An [`AutoPoll`] with a no-op closure and a throwaway id, for tests
    /// that only need an entry in the map.
    pub(crate) fn for_test(interval_id: i32) -> Self {
        Self {
            interval_id,
            _keep_alive: wasm_bindgen::prelude::Closure::<dyn FnMut()>::new(|| {}),
        }
    }
}

/// Applies `preset` to one `RTCRtpSender` encoding: bitrate ceiling,
/// resolution scale, and frame-rate cap. web-sys has no typed setter for
/// `maxFramerate`, so it goes straight onto the encoding dict; the browser
/// reads it there. `degradationPreference` (drop frames vs. resolution
/// under pressure) is *not* set here — it's the sharer's `VideoMode`
/// choice, owned by `session::video_mode` and applied over the top of this.
#[cfg(feature = "hydrate")]
fn configure_encoding(encoding: &web_sys::RtcRtpEncodingParameters, preset: EncodingPreset) {
    use wasm_bindgen::JsValue;

    encoding.set_max_bitrate(preset.max_bitrate_bps);
    encoding.set_scale_resolution_down_by(preset.scale_down);
    let _ = js_sys::Reflect::set(
        encoding,
        &JsValue::from_str("maxFramerate"),
        &JsValue::from_f64(preset.max_framerate),
    );
}

/// Finds the video `RTCRtpSender` on `pc` (there's at most one — this app
/// never sends more than a single video track per connection) and pins it
/// to `tier`'s bitrate/scale/frame-rate. A no-op if sharing hasn't actually
/// started yet (no sender), which can happen if a quality change races the
/// initial track being added.
#[cfg(feature = "hydrate")]
pub(crate) async fn apply_tier(
    pc: &web_sys::RtcPeerConnection,
    tier: Tier,
) -> Result<(), wasm_bindgen::JsValue> {
    use wasm_bindgen::JsCast;
    use wasm_bindgen_futures::JsFuture;

    let sender = pc.get_senders().iter().find_map(|entry| {
        let sender: web_sys::RtcRtpSender = entry.unchecked_into();
        let is_video = sender.track().is_some_and(|track| track.kind() == "video");
        is_video.then_some(sender)
    });
    let Some(sender) = sender else { return Ok(()) };

    let params = sender.get_parameters();
    let encodings = params.get_encodings().unwrap_or_else(js_sys::Array::new);
    let preset = preset_for(tier);

    if encodings.length() == 0 {
        let encoding = web_sys::RtcRtpEncodingParameters::new();
        configure_encoding(&encoding, preset);
        encodings.push(&encoding);
    } else {
        for entry in encodings.iter() {
            let encoding: web_sys::RtcRtpEncodingParameters = entry.unchecked_into();
            configure_encoding(&encoding, preset);
        }
    }
    params.set_encodings(&encodings);

    JsFuture::from(sender.set_parameters_with_parameters(&params)).await?;
    Ok(())
}

/// One `getStats()` poll for `pc`'s video sender, reduced to what
/// `AdaptiveQuality` needs. `None` if there's no outbound video yet (sender
/// not established, or its first stats haven't landed) — the caller should
/// just skip that tick rather than feed a monitor a reading that means
/// nothing.
#[cfg(feature = "hydrate")]
async fn read_reading(pc: &web_sys::RtcPeerConnection) -> Option<RawReading> {
    use wasm_bindgen::JsCast;
    use wasm_bindgen_futures::JsFuture;

    let report = JsFuture::from(pc.get_stats()).await.ok()?;
    let report: js_sys::Map = report.unchecked_into();

    let mut limitation_is_bad = false;
    let mut packets_sent_total = None;
    let mut packets_lost_total = None;

    report.for_each(&mut |value, _key| {
        let field =
            |name: &str| js_sys::Reflect::get(&value, &wasm_bindgen::JsValue::from_str(name)).ok();
        let Some(stat_type) = field("type").and_then(|v| v.as_string()) else {
            return;
        };
        let kind = field("kind").and_then(|v| v.as_string());

        match stat_type.as_str() {
            "outbound-rtp" if kind.as_deref() == Some("video") => {
                if let Some(reason) = field("qualityLimitationReason").and_then(|v| v.as_string()) {
                    limitation_is_bad = reason == "bandwidth" || reason == "cpu";
                }
                packets_sent_total = field("packetsSent").and_then(|v| v.as_f64());
            }
            // Reported back over RTCP by the viewer — absent until their
            // first receiver report arrives, which is fine: `unwrap_or` to
            // "no loss yet" rather than treating it as no signal at all.
            "remote-inbound-rtp" if kind.as_deref() == Some("video") => {
                packets_lost_total = field("packetsLost").and_then(|v| v.as_f64());
            }
            _ => {}
        }
    });

    Some(RawReading {
        limitation_is_bad,
        packets_sent_total: packets_sent_total?,
        packets_lost_total: packets_lost_total.unwrap_or(0.0),
    })
}

/// Starts polling `viewer_peer_id`'s connection and adapting its quality
/// every `AUTO_POLL_INTERVAL_MS`. `initial` decides whether `Tier::High` is
/// applied up front (see [`InitialTier`]); either way a fresh
/// `AdaptiveQuality` assumes `High`, so the monitor and the sender agree
/// from the first tick. Idempotent-ish: callers should `stop_auto_polling`
/// first if one might already be running for this viewer, or two intervals
/// end up fighting over the same sender.
#[cfg(feature = "hydrate")]
pub(crate) fn start_auto_polling(
    conn: crate::session::RoomSession,
    viewer_peer_id: String,
    initial: InitialTier,
) {
    use std::cell::RefCell;
    use std::rc::Rc;

    use leptos::task::spawn_local;
    use wasm_bindgen::prelude::Closure;
    use wasm_bindgen::JsCast;

    let Some(window) = web_sys::window() else {
        return;
    };

    let monitor = Rc::new(RefCell::new(AdaptiveQuality::new()));

    // Nothing to poll if the connection isn't up yet.
    if !conn.outgoing.borrow().contains_key(&viewer_peer_id) {
        return;
    }

    if let InitialTier::ResetToHigh = initial {
        if let Some(pc) = conn.outgoing.borrow().get(&viewer_peer_id).cloned() {
            let starting_tier = monitor.borrow().tier();
            spawn_local(async move {
                let _ = apply_tier(&pc, starting_tier).await;
            });
        }
    }
    let on_tick = Closure::<dyn FnMut()>::new({
        let conn = conn.clone();
        let viewer_peer_id = viewer_peer_id.clone();
        move || {
            let conn = conn.clone();
            let viewer_peer_id = viewer_peer_id.clone();
            let monitor = monitor.clone();
            spawn_local(async move {
                let Some(pc) = conn.outgoing.borrow().get(&viewer_peer_id).cloned() else {
                    return;
                };
                let Some(reading) = read_reading(&pc).await else {
                    return;
                };
                let Some(new_tier) = monitor.borrow_mut().record_reading(reading) else {
                    return;
                };
                let _ = apply_tier(&pc, new_tier).await;
            });
        }
    });

    let Ok(interval_id) = window.set_interval_with_callback_and_timeout_and_arguments_0(
        on_tick.as_ref().unchecked_ref(),
        AUTO_POLL_INTERVAL_MS,
    ) else {
        return;
    };

    conn.quality_auto_intervals.borrow_mut().insert(
        viewer_peer_id,
        AutoPoll {
            interval_id,
            _keep_alive: on_tick,
        },
    );
}

/// Whether an Auto quality poll is currently registered for `viewer_peer_id`.
/// True only while that viewer's quality is `Auto`: picking a fixed tier
/// runs [`stop_auto_polling`], so this doubles as "is this viewer still
/// being adapted?" — used after renegotiation to decide whether re-asserting
/// `Tier::High` would stomp a deliberate fixed-tier choice.
#[cfg(feature = "hydrate")]
pub(crate) fn is_auto_polling(conn: &crate::session::RoomSession, viewer_peer_id: &str) -> bool {
    conn.quality_auto_intervals
        .borrow()
        .contains_key(viewer_peer_id)
}

/// Stops `viewer_peer_id`'s Auto poll if one is running — safe to call even
/// if there isn't one (switching between two fixed tiers, e.g.).
#[cfg(feature = "hydrate")]
pub(crate) fn stop_auto_polling(conn: &crate::session::RoomSession, viewer_peer_id: &str) {
    let Some(poll) = conn
        .quality_auto_intervals
        .borrow_mut()
        .remove(viewer_peer_id)
    else {
        return;
    };
    clear(poll);
}

/// Stops every registered Auto poll. Used when the whole session goes
/// away — leaving the room (see `stop_auto_polling_on_cleanup`), a
/// reconnect, or the local share stopping — where per-viewer bookkeeping
/// is pointless and leaving the intervals running keeps the `RoomSession`
/// graph alive.
#[cfg(feature = "hydrate")]
pub(crate) fn stop_all_auto_polling(conn: &crate::session::RoomSession) {
    let polls: Vec<AutoPoll> = conn
        .quality_auto_intervals
        .borrow_mut()
        .drain()
        .map(|(_, poll)| poll)
        .collect();
    for poll in polls {
        clear(poll);
    }
}

/// `clearInterval` for `poll`; its `_keep_alive` closure then drops.
#[cfg(feature = "hydrate")]
fn clear(poll: AutoPoll) {
    if let Some(window) = web_sys::window() {
        window.clear_interval_with_handle(poll.interval_id);
    }
}

/// The viewer-side half: sends a chosen quality for the member at `slot` to
/// that member (the sharer), who applies it to the one connection it's for
/// — see `ServerMessage::QualityRequested` in `message_handler.rs`. Reads
/// `members`/`slot` at call time rather than closing over a fixed peer_id,
/// same as `watch_click_handler`, since a slot's occupant can change.
#[cfg(not(feature = "hydrate"))]
pub(crate) fn set_quality_handler(
    _conn: crate::session::RoomSession,
    _members: leptos::prelude::ReadSignal<Vec<crate::session::RoomMember>>,
    _quality_by_peer: leptos::prelude::RwSignal<
        std::collections::HashMap<String, screen_share_protocol::QualityLevel>,
    >,
    _slot: usize,
) -> impl Fn(screen_share_protocol::QualityLevel) + Clone + 'static {
    move |_| {}
}

#[cfg(feature = "hydrate")]
pub(crate) fn set_quality_handler(
    conn: crate::session::RoomSession,
    members: leptos::prelude::ReadSignal<Vec<crate::session::RoomMember>>,
    quality_by_peer: leptos::prelude::RwSignal<
        std::collections::HashMap<String, screen_share_protocol::QualityLevel>,
    >,
    slot: usize,
) -> impl Fn(screen_share_protocol::QualityLevel) + Clone + 'static {
    use leptos::prelude::*;

    move |quality| {
        let Some(member) = members.get_untracked().get(slot).cloned() else {
            return;
        };
        quality_by_peer.update(|m| {
            m.insert(member.peer_id.clone(), quality);
        });
        if let Some(ws) = conn.ws.borrow().as_ref() {
            ws.send(&screen_share_protocol::ClientMessage::SetQuality {
                to: member.peer_id,
                quality,
            });
        }
    }
}

#[cfg(all(test, target_arch = "wasm32", feature = "hydrate"))]
#[path = "wasm_tests.rs"]
mod wasm_tests;
