//! The bandwidth-adaptive quality state machine behind
//! `QualityLevel::Auto`, and the per-tier encoding presets — the pure,
//! browser-free half of the web app's per-viewer quality control. The
//! `getStats()` / `setParameters()` mechanics that feed and apply this
//! live in `apps/web` (`session::quality`), which turns a real
//! `RTCRtpSender` stats report into a [`RawReading`] and applies an
//! [`EncodingPreset`] to a real encoding.

/// One quality tier `RTCRtpSender.setParameters()` can be pinned to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tier {
    High,
    Medium,
    Low,
}

impl Tier {
    /// The next tier down, saturating at [`Tier::Low`].
    #[must_use]
    pub fn step_down(self) -> Self {
        match self {
            Tier::High => Tier::Medium,
            Tier::Medium | Tier::Low => Tier::Low,
        }
    }

    /// The next tier up, saturating at [`Tier::High`].
    #[must_use]
    pub fn step_up(self) -> Self {
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
const HIGH_MAX_BITRATE_BPS: u32 = 4_000_000;
const MEDIUM_MAX_BITRATE_BPS: u32 = 1_200_000;
const LOW_MAX_BITRATE_BPS: u32 = 400_000;

/// `scaleResolutionDownBy` per tier — 1.0 keeps native resolution, higher
/// values shrink the encoded frame so a starved connection stays legible.
const HIGH_SCALE_DOWN: f32 = 1.0;
const MEDIUM_SCALE_DOWN: f32 = 1.5;
const LOW_SCALE_DOWN: f32 = 3.0;

/// `maxFramerate` per tier. The top tier allows a full 60 so a member
/// sharing video or a game gets smooth motion; `contentHint = "detail"` on
/// the captured track plus `degradationPreference = "maintain-resolution"`
/// (set together in `session::quality::configure_encoding`) keep a
/// mostly-static screen from spending bitrate on frames it doesn't need.
/// The lower tiers cap motion hard so a squeezed connection puts its
/// budget into staying sharp, not fluid.
const HIGH_MAX_FRAMERATE: f64 = 60.0;
const MEDIUM_MAX_FRAMERATE: f64 = 30.0;
const LOW_MAX_FRAMERATE: f64 = 15.0;

/// The `RTCRtpSender` encoding knobs one quality tier pins.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EncodingPreset {
    pub max_bitrate_bps: u32,
    pub scale_down: f32,
    pub max_framerate: f64,
}

/// The [`EncodingPreset`] for a tier.
#[must_use]
pub fn preset_for(tier: Tier) -> EncodingPreset {
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

/// The fixed [`Tier`] for a `QualityLevel`, or `None` for
/// `QualityLevel::Auto` — it has no single tier of its own, it's
/// [`AdaptiveQuality`] picking one continuously.
#[must_use]
pub fn tier_for(level: screen_share_protocol::QualityLevel) -> Option<Tier> {
    use screen_share_protocol::QualityLevel;
    match level {
        QualityLevel::Auto => None,
        QualityLevel::High => Some(Tier::High),
        QualityLevel::Medium => Some(Tier::Medium),
        QualityLevel::Low => Some(Tier::Low),
    }
}

/// Whether the sharer should pin the sender to [`Tier::High`] before the
/// first poll of a fresh Auto run.
pub enum InitialTier {
    /// Re-apply `High` now — the sender may currently be pinned to a lower
    /// tier (a manual switch back to `Auto`).
    ResetToHigh,
    /// The caller already established the encoding this run (and re-asserted
    /// the video mode over it); skip the redundant apply so it can't race
    /// the offer that's about to be built.
    AlreadyApplied,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Signal {
    Good,
    Bad,
    Neutral,
}

/// How many consecutive `Bad` polls before dropping a tier — more than one,
/// so a single blip doesn't cause a step; kept low because a bandwidth
/// squeeze is worth reacting to quickly.
const BAD_STREAK_TO_STEP_DOWN: u32 = 2;
/// How many consecutive `Good` polls before raising a tier — higher than
/// the step-down threshold: recovering bandwidth should be trusted for
/// longer than losing it, or the tier flaps on a connection that's merely
/// borderline.
const GOOD_STREAK_TO_STEP_UP: u32 = 3;
/// Packet-loss ratio (since the last poll, not cumulative — see
/// [`AdaptiveQuality::classify`]) above which a poll counts as `Bad`.
const BAD_LOSS_RATIO: f64 = 0.03;
/// Packet-loss ratio below which a poll counts as `Good`. Left a gap
/// between this and `BAD_LOSS_RATIO` (0.5%..3%) that reads as `Neutral` —
/// without it, noise right at one threshold would flap the streak counters
/// every other poll instead of actually settling on a verdict.
const GOOD_LOSS_RATIO: f64 = 0.005;

/// One `getStats()` poll's worth of raw signal, in the cumulative form
/// WebRTC reports it — [`AdaptiveQuality`] turns consecutive readings into
/// a per-interval delta itself, since the cumulative totals alone dilute a
/// fresh problem into an ever-growing denominator over a long session.
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
pub struct AdaptiveQuality {
    tier: Tier,
    bad_streak: u32,
    good_streak: u32,
    previous_reading: Option<RawReading>,
}

impl AdaptiveQuality {
    #[must_use]
    pub fn new() -> Self {
        Self {
            tier: Tier::High,
            bad_streak: 0,
            good_streak: 0,
            previous_reading: None,
        }
    }

    #[must_use]
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

impl Default for AdaptiveQuality {
    fn default() -> Self {
        Self::new()
    }
}
