//! Per-viewer video quality: the sharer-side `getStats()`/`setParameters()`
//! mechanics, the bandwidth-adaptive hysteresis behind `QualityLevel::Auto`
//! (kept pure and browser-free below so it's unit-testable on its own), and
//! the viewer-side click handler that sends a chosen quality.

// This whole section (through `AdaptiveQuality` and its `impl`) is plain
// Rust with no `web-sys` — `cfg(any(test, feature = "hydrate"))`, not just
// `hydrate`, avoids dead-code warnings on an `ssr`-only build (its only
// real caller is hydrate-gated) while keeping it testable without a
// browser. Same reasoning as `extract_room_code` in `home/join_room.rs`.

/// One quality tier `RTCRtpSender.setParameters()` can be pinned to.
#[cfg(any(test, feature = "hydrate"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tier {
    High,
    Medium,
    Low,
}

#[cfg(any(test, feature = "hydrate"))]
impl Tier {
    fn step_down(self) -> Self {
        match self {
            Tier::High => Tier::Medium,
            Tier::Medium | Tier::Low => Tier::Low,
        }
    }

    fn step_up(self) -> Self {
        match self {
            Tier::Low => Tier::Medium,
            Tier::Medium | Tier::High => Tier::High,
        }
    }
}

/// maxBitrate in bps per tier. Screen-share content (sharp text/UI edges)
/// needs a much higher bitrate per pixel than typical webcam presets before
/// it looks blocky, so these run well above generic WebRTC quality-preset
/// numbers found elsewhere.
#[cfg(any(test, feature = "hydrate"))]
const HIGH_MAX_BITRATE_BPS: u32 = 4_000_000;
#[cfg(any(test, feature = "hydrate"))]
const MEDIUM_MAX_BITRATE_BPS: u32 = 1_200_000;
#[cfg(any(test, feature = "hydrate"))]
const LOW_MAX_BITRATE_BPS: u32 = 400_000;

/// `scaleResolutionDownBy` per tier — 1.0 keeps native resolution, higher
/// values shrink the encoded frame so a starved connection stays legible.
#[cfg(any(test, feature = "hydrate"))]
const HIGH_SCALE_DOWN: f32 = 1.0;
#[cfg(any(test, feature = "hydrate"))]
const MEDIUM_SCALE_DOWN: f32 = 1.5;
#[cfg(any(test, feature = "hydrate"))]
const LOW_SCALE_DOWN: f32 = 3.0;

/// `maxFramerate` per tier. The top tier allows a full 60 so a member
/// sharing video or a game gets smooth motion; `contentHint = "detail"` on
/// the captured track plus `degradationPreference = "maintain-resolution"`
/// (set together in `configure_encoding`) keep a mostly-static screen from
/// spending bitrate on frames it doesn't need. The lower tiers cap motion
/// hard so a squeezed connection puts its budget into staying sharp, not
/// fluid.
#[cfg(any(test, feature = "hydrate"))]
const HIGH_MAX_FRAMERATE: f64 = 60.0;
#[cfg(any(test, feature = "hydrate"))]
const MEDIUM_MAX_FRAMERATE: f64 = 30.0;
#[cfg(any(test, feature = "hydrate"))]
const LOW_MAX_FRAMERATE: f64 = 15.0;

/// The `RTCRtpSender` encoding knobs one quality tier pins.
#[cfg(any(test, feature = "hydrate"))]
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct EncodingPreset {
    pub max_bitrate_bps: u32,
    pub scale_down: f32,
    pub max_framerate: f64,
}

#[cfg(any(test, feature = "hydrate"))]
fn preset_for(tier: Tier) -> EncodingPreset {
    match tier {
        Tier::High => EncodingPreset {
            max_bitrate_bps: HIGH_MAX_BITRATE_BPS,
            scale_down: HIGH_SCALE_DOWN,
            max_framerate: HIGH_MAX_FRAMERATE,
        },
        Tier::Medium => EncodingPreset {
            max_bitrate_bps: MEDIUM_MAX_BITRATE_BPS,
            scale_down: MEDIUM_SCALE_DOWN,
            max_framerate: MEDIUM_MAX_FRAMERATE,
        },
        Tier::Low => EncodingPreset {
            max_bitrate_bps: LOW_MAX_BITRATE_BPS,
            scale_down: LOW_SCALE_DOWN,
            max_framerate: LOW_MAX_FRAMERATE,
        },
    }
}

/// `None` for `QualityLevel::Auto` — it has no single tier of its own, it's
/// `AdaptiveQuality` picking one continuously.
#[cfg(any(test, feature = "hydrate"))]
pub(crate) fn tier_for(level: screen_share_protocol::QualityLevel) -> Option<Tier> {
    use screen_share_protocol::QualityLevel;
    match level {
        QualityLevel::Auto => None,
        QualityLevel::High => Some(Tier::High),
        QualityLevel::Medium => Some(Tier::Medium),
        QualityLevel::Low => Some(Tier::Low),
    }
}

#[cfg(any(test, feature = "hydrate"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Signal {
    Good,
    Bad,
    Neutral,
}

/// How many consecutive `Bad` polls before dropping a tier — more than one,
/// so a single blip doesn't cause a step; kept low because a bandwidth
/// squeeze is worth reacting to quickly.
#[cfg(any(test, feature = "hydrate"))]
const BAD_STREAK_TO_STEP_DOWN: u32 = 2;
/// How many consecutive `Good` polls before raising a tier — higher than
/// the step-down threshold: recovering bandwidth should be trusted for
/// longer than losing it, or the tier flaps on a connection that's merely
/// borderline.
#[cfg(any(test, feature = "hydrate"))]
const GOOD_STREAK_TO_STEP_UP: u32 = 3;
/// Packet-loss ratio (since the last poll, not cumulative — see
/// `AdaptiveQuality::classify`) above which a poll counts as `Bad`.
#[cfg(any(test, feature = "hydrate"))]
const BAD_LOSS_RATIO: f64 = 0.03;
/// Packet-loss ratio below which a poll counts as `Good`. Left a gap
/// between this and `BAD_LOSS_RATIO` (0.5%..3%) that reads as `Neutral` —
/// without it, noise right at one threshold would flap the streak counters
/// every other poll instead of actually settling on a verdict.
#[cfg(any(test, feature = "hydrate"))]
const GOOD_LOSS_RATIO: f64 = 0.005;

/// One `getStats()` poll's worth of raw signal, in the cumulative form
/// WebRTC reports it — `AdaptiveQuality` turns consecutive readings into a
/// per-interval delta itself, since the cumulative totals alone dilute a
/// fresh problem into an ever-growing denominator over a long session.
#[cfg(any(test, feature = "hydrate"))]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RawReading {
    /// `true` if the sender's own `qualityLimitationReason` was
    /// `"bandwidth"` or `"cpu"` at poll time.
    pub limitation_is_bad: bool,
    pub packets_sent_total: f64,
    pub packets_lost_total: f64,
}

/// Bandwidth-adaptive tier picker for one (sharer, viewer) connection.
/// Starts at `High` and steps down/up based on a hysteresis over recent
/// `getStats()` polls — see the constants above for exactly how much
/// evidence it takes to move.
#[cfg(any(test, feature = "hydrate"))]
pub struct AdaptiveQuality {
    tier: Tier,
    bad_streak: u32,
    good_streak: u32,
    previous_reading: Option<RawReading>,
}

#[cfg(any(test, feature = "hydrate"))]
impl AdaptiveQuality {
    pub fn new() -> Self {
        Self {
            tier: Tier::High,
            bad_streak: 0,
            good_streak: 0,
            previous_reading: None,
        }
    }

    pub fn tier(&self) -> Tier {
        self.tier
    }

    /// Feeds in one poll; returns `Some(new_tier)` only on the poll where
    /// the tier actually changes, so callers can tell "nothing to do" apart
    /// from "re-apply the same tier".
    pub fn record_reading(&mut self, reading: RawReading) -> Option<Tier> {
        let signal = self.classify(reading);
        self.previous_reading = Some(reading);
        self.record_signal(signal)
    }

    /// `None` (first poll, nothing to compare against) reads as `Neutral`.
    fn classify(&self, reading: RawReading) -> Signal {
        let Some(prev) = self.previous_reading else {
            return Signal::Neutral;
        };
        let sent_delta = (reading.packets_sent_total - prev.packets_sent_total).max(0.0);
        let lost_delta = (reading.packets_lost_total - prev.packets_lost_total).max(0.0);
        let loss_ratio = if sent_delta > 0.0 {
            lost_delta / sent_delta
        } else {
            0.0
        };

        if reading.limitation_is_bad || loss_ratio > BAD_LOSS_RATIO {
            Signal::Bad
        } else if loss_ratio < GOOD_LOSS_RATIO {
            Signal::Good
        } else {
            Signal::Neutral
        }
    }

    fn record_signal(&mut self, signal: Signal) -> Option<Tier> {
        match signal {
            Signal::Bad => {
                self.good_streak = 0;
                self.bad_streak += 1;
                if self.bad_streak < BAD_STREAK_TO_STEP_DOWN {
                    return None;
                }
                self.bad_streak = 0;
                self.step(Tier::step_down)
            }
            Signal::Good => {
                self.bad_streak = 0;
                self.good_streak += 1;
                if self.good_streak < GOOD_STREAK_TO_STEP_UP {
                    return None;
                }
                self.good_streak = 0;
                self.step(Tier::step_up)
            }
            Signal::Neutral => {
                self.bad_streak = 0;
                self.good_streak = 0;
                None
            }
        }
    }

    fn step(&mut self, direction: fn(Tier) -> Tier) -> Option<Tier> {
        let next = direction(self.tier);
        (next != self.tier).then(|| {
            self.tier = next;
            next
        })
    }
}

#[cfg(any(test, feature = "hydrate"))]
impl Default for AdaptiveQuality {
    fn default() -> Self {
        Self::new()
    }
}

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

/// Whether [`start_auto_polling`] should pin the sender to `Tier::High`
/// before its first poll.
#[cfg(feature = "hydrate")]
pub(crate) enum InitialTier {
    /// Re-apply `High` now — the sender may currently be pinned to a lower
    /// tier (a manual switch back to `Auto`).
    ResetToHigh,
    /// The caller already established the encoding this run (and re-asserted
    /// the video mode over it); skip the redundant apply so it can't race
    /// the offer that's about to be built.
    AlreadyApplied,
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

#[cfg(test)]
#[path = "tests.rs"]
mod tests;

#[cfg(all(test, target_arch = "wasm32", feature = "hydrate"))]
#[path = "wasm_tests.rs"]
mod wasm_tests;
