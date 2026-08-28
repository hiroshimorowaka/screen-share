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

/// (maxBitrate in bps, scaleResolutionDownBy) per tier. Screen-share content
/// (sharp text/UI edges) needs a much higher bitrate per pixel than typical
/// webcam presets before it looks blocky, so these run well above generic
/// WebRTC quality-preset numbers found elsewhere.
#[cfg(any(test, feature = "hydrate"))]
const HIGH_MAX_BITRATE_BPS: u32 = 4_000_000;
#[cfg(any(test, feature = "hydrate"))]
const MEDIUM_MAX_BITRATE_BPS: u32 = 1_200_000;
#[cfg(any(test, feature = "hydrate"))]
const LOW_MAX_BITRATE_BPS: u32 = 400_000;

#[cfg(any(test, feature = "hydrate"))]
fn preset_for(tier: Tier) -> (u32, f32) {
    match tier {
        Tier::High => (HIGH_MAX_BITRATE_BPS, 1.0),
        Tier::Medium => (MEDIUM_MAX_BITRATE_BPS, 1.5),
        Tier::Low => (LOW_MAX_BITRATE_BPS, 3.0),
    }
}

/// `None` for `QualityLevel::Auto` — it has no single tier of its own, it's
/// `AdaptiveQuality` picking one continuously.
#[cfg(any(test, feature = "hydrate"))]
pub(super) fn tier_for(level: crate::signaling::protocol::QualityLevel) -> Option<Tier> {
    use crate::signaling::protocol::QualityLevel;
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
        Self { tier: Tier::High, bad_streak: 0, good_streak: 0, previous_reading: None }
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
        let Some(prev) = self.previous_reading else { return Signal::Neutral };
        let sent_delta = (reading.packets_sent_total - prev.packets_sent_total).max(0.0);
        let lost_delta = (reading.packets_lost_total - prev.packets_lost_total).max(0.0);
        let loss_ratio = if sent_delta > 0.0 { lost_delta / sent_delta } else { 0.0 };

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

/// Finds the video `RTCRtpSender` on `pc` (there's at most one — this app
/// never sends more than a single video track per connection) and pins it
/// to `tier`'s bitrate/scale. A no-op if sharing hasn't actually started
/// yet (no sender), which can happen if a quality change races the initial
/// track being added.
#[cfg(feature = "hydrate")]
pub(super) async fn apply_tier(pc: &web_sys::RtcPeerConnection, tier: Tier) -> Result<(), wasm_bindgen::JsValue> {
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
    let (max_bitrate, scale) = preset_for(tier);

    if encodings.length() == 0 {
        let encoding = web_sys::RtcRtpEncodingParameters::new();
        encoding.set_max_bitrate(max_bitrate);
        encoding.set_scale_resolution_down_by(scale);
        encodings.push(&encoding);
    } else {
        for entry in encodings.iter() {
            let encoding: web_sys::RtcRtpEncodingParameters = entry.unchecked_into();
            encoding.set_max_bitrate(max_bitrate);
            encoding.set_scale_resolution_down_by(scale);
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
        let field = |name: &str| js_sys::Reflect::get(&value, &wasm_bindgen::JsValue::from_str(name)).ok();
        let Some(stat_type) = field("type").and_then(|v| v.as_string()) else { return };
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

/// Starts polling `viewer_peer_id`'s connection and adapting its quality —
/// applies `Tier::High` immediately (matching a fresh `AdaptiveQuality`'s
/// starting assumption) so the sender and the monitor agree on the tier
/// from the first tick, then re-evaluates every `AUTO_POLL_INTERVAL_MS`.
/// Idempotent-ish: callers should `stop_auto_polling` first if one might
/// already be running for this viewer, or two intervals end up fighting
/// over the same sender.
#[cfg(feature = "hydrate")]
pub(super) fn start_auto_polling(conn: super::connection::RoomConnection, viewer_peer_id: String) {
    use std::cell::RefCell;
    use std::rc::Rc;

    use leptos::task::spawn_local;
    use wasm_bindgen::prelude::Closure;
    use wasm_bindgen::JsCast;

    let Some(window) = web_sys::window() else { return };

    let monitor = Rc::new(RefCell::new(AdaptiveQuality::new()));

    let Some(pc) = conn.outgoing.borrow().get(&viewer_peer_id).cloned() else { return };
    let starting_tier = monitor.borrow().tier();
    spawn_local(async move {
        let _ = apply_tier(&pc, starting_tier).await;
    });
    let on_tick = Closure::<dyn FnMut()>::new({
        let conn = conn.clone();
        let viewer_peer_id = viewer_peer_id.clone();
        move || {
            let conn = conn.clone();
            let viewer_peer_id = viewer_peer_id.clone();
            let monitor = monitor.clone();
            spawn_local(async move {
                let Some(pc) = conn.outgoing.borrow().get(&viewer_peer_id).cloned() else { return };
                let Some(reading) = read_reading(&pc).await else { return };
                let Some(new_tier) = monitor.borrow_mut().record_reading(reading) else { return };
                let _ = apply_tier(&pc, new_tier).await;
            });
        }
    });

    let Ok(interval_id) =
        window.set_interval_with_callback_and_timeout_and_arguments_0(on_tick.as_ref().unchecked_ref(), AUTO_POLL_INTERVAL_MS)
    else {
        return;
    };
    on_tick.forget();

    conn.quality_auto_intervals.borrow_mut().insert(viewer_peer_id, interval_id);
}

/// Stops `viewer_peer_id`'s Auto poll if one is running — safe to call even
/// if there isn't one (switching between two fixed tiers, e.g.).
#[cfg(feature = "hydrate")]
pub(super) fn stop_auto_polling(conn: &super::connection::RoomConnection, viewer_peer_id: &str) {
    let Some(interval_id) = conn.quality_auto_intervals.borrow_mut().remove(viewer_peer_id) else { return };
    if let Some(window) = web_sys::window() {
        window.clear_interval_with_handle(interval_id);
    }
}

/// The viewer-side half: sends a chosen quality for the member at `slot` to
/// that member (the sharer), who applies it to the one connection it's for
/// — see `ServerMessage::QualityRequested` in `message_handler.rs`. Reads
/// `members`/`slot` at call time rather than closing over a fixed peer_id,
/// same as `watch_click_handler`, since a slot's occupant can change.
#[cfg(not(feature = "hydrate"))]
pub(super) fn set_quality_handler(
    _conn: super::connection::RoomConnection,
    _members: leptos::prelude::ReadSignal<Vec<super::RoomMember>>,
    _quality_by_peer: leptos::prelude::RwSignal<
        std::collections::HashMap<String, crate::signaling::protocol::QualityLevel>,
    >,
    _slot: usize,
) -> impl Fn(crate::signaling::protocol::QualityLevel) + Clone + 'static {
    move |_| {}
}

#[cfg(feature = "hydrate")]
pub(super) fn set_quality_handler(
    conn: super::connection::RoomConnection,
    members: leptos::prelude::ReadSignal<Vec<super::RoomMember>>,
    quality_by_peer: leptos::prelude::RwSignal<
        std::collections::HashMap<String, crate::signaling::protocol::QualityLevel>,
    >,
    slot: usize,
) -> impl Fn(crate::signaling::protocol::QualityLevel) + Clone + 'static {
    use leptos::prelude::*;

    move |quality| {
        let Some(member) = members.get_untracked().get(slot).cloned() else { return };
        quality_by_peer.update(|m| {
            m.insert(member.peer_id.clone(), quality);
        });
        if let Some(ws) = conn.ws.borrow().as_ref() {
            ws.send(&crate::signaling::protocol::ClientMessage::SetQuality { to: member.peer_id, quality });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reading(limitation_is_bad: bool, sent_total: f64, lost_total: f64) -> RawReading {
        RawReading { limitation_is_bad, packets_sent_total: sent_total, packets_lost_total: lost_total }
    }

    #[test]
    fn preset_bitrate_strictly_decreases_from_high_to_low() {
        let (high, _) = preset_for(Tier::High);
        let (medium, _) = preset_for(Tier::Medium);
        let (low, _) = preset_for(Tier::Low);
        assert!(high > medium && medium > low);
    }

    #[test]
    fn preset_scale_down_never_decreases_from_high_to_low() {
        let (_, high) = preset_for(Tier::High);
        let (_, medium) = preset_for(Tier::Medium);
        let (_, low) = preset_for(Tier::Low);
        assert!(high <= medium && medium <= low);
    }

    #[test]
    fn tier_for_maps_every_fixed_level_and_leaves_auto_as_none() {
        use crate::signaling::protocol::QualityLevel;
        assert_eq!(tier_for(QualityLevel::Auto), None);
        assert_eq!(tier_for(QualityLevel::High), Some(Tier::High));
        assert_eq!(tier_for(QualityLevel::Medium), Some(Tier::Medium));
        assert_eq!(tier_for(QualityLevel::Low), Some(Tier::Low));
    }

    #[test]
    fn starts_at_high() {
        assert_eq!(AdaptiveQuality::new().tier(), Tier::High);
    }

    #[test]
    fn first_reading_never_moves_the_tier() {
        let mut q = AdaptiveQuality::new();
        assert_eq!(q.record_reading(reading(true, 1000.0, 900.0)), None);
        assert_eq!(q.tier(), Tier::High);
    }

    #[test]
    fn steps_down_after_two_consecutive_bad_readings_via_limitation_reason() {
        let mut q = AdaptiveQuality::new();
        q.record_reading(reading(false, 0.0, 0.0));
        assert_eq!(q.record_reading(reading(true, 100.0, 0.0)), None, "one bad reading alone shouldn't step");
        assert_eq!(q.record_reading(reading(true, 200.0, 0.0)), Some(Tier::Medium));
    }

    #[test]
    fn steps_down_from_high_loss_ratio_even_without_a_limitation_reason() {
        let mut q = AdaptiveQuality::new();
        q.record_reading(reading(false, 0.0, 0.0));
        // 10% loss this interval (10/100), well above BAD_LOSS_RATIO.
        q.record_reading(reading(false, 100.0, 10.0));
        assert_eq!(q.record_reading(reading(false, 200.0, 20.0)), Some(Tier::Medium));
    }

    #[test]
    fn a_single_good_reading_between_bad_ones_resets_the_bad_streak() {
        let mut q = AdaptiveQuality::new();
        q.record_reading(reading(false, 0.0, 0.0));
        q.record_reading(reading(true, 100.0, 0.0));
        // Clean interval: 0% loss, no limitation — counts as Good, not just
        // "not bad", and must zero the bad streak so far.
        q.record_reading(reading(false, 200.0, 0.0));
        assert_eq!(q.record_reading(reading(true, 300.0, 0.0)), None, "the earlier bad streak must not have carried over");
    }

    #[test]
    fn steps_down_twice_reaches_low_not_past_it() {
        let mut q = AdaptiveQuality::new();
        q.record_reading(reading(false, 0.0, 0.0));
        q.record_reading(reading(true, 100.0, 0.0));
        assert_eq!(q.record_reading(reading(true, 200.0, 0.0)), Some(Tier::Medium));
        q.record_reading(reading(true, 300.0, 0.0));
        assert_eq!(q.record_reading(reading(true, 400.0, 0.0)), Some(Tier::Low));
        // A third drop has nowhere to go.
        q.record_reading(reading(true, 500.0, 0.0));
        assert_eq!(q.record_reading(reading(true, 600.0, 0.0)), None);
        assert_eq!(q.tier(), Tier::Low);
    }

    #[test]
    fn steps_up_after_three_consecutive_good_readings() {
        let mut q = AdaptiveQuality::new();
        q.record_reading(reading(false, 0.0, 0.0));
        q.record_reading(reading(true, 100.0, 0.0));
        q.record_reading(reading(true, 200.0, 0.0)); // now Medium
        assert_eq!(q.tier(), Tier::Medium);

        q.record_reading(reading(false, 300.0, 0.0));
        assert_eq!(q.record_reading(reading(false, 400.0, 0.0)), None, "two good readings alone shouldn't step up yet");
        assert_eq!(q.record_reading(reading(false, 500.0, 0.0)), Some(Tier::High));
    }

    #[test]
    fn moderate_loss_between_the_thresholds_is_neutral_and_does_not_accumulate() {
        let mut q = AdaptiveQuality::new();
        q.record_reading(reading(false, 0.0, 0.0));
        // 1% loss: below BAD_LOSS_RATIO, above GOOD_LOSS_RATIO — neutral.
        for i in 1..10 {
            let sent = (i as f64) * 100.0;
            assert_eq!(q.record_reading(reading(false, sent, sent * 0.01)), None);
        }
        assert_eq!(q.tier(), Tier::High, "sustained neutral readings should never move the tier");
    }
}
