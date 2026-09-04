//! Unit tests for `quality`, split out of src/quality.rs to keep it
//! readable (refactor Phase 4). Kept in-crate: they exercise items that
//! are not part of the crate's public API.

use screen_share_domain::quality::*;

fn reading(limitation_is_bad: bool, sent_total: f64, lost_total: f64) -> RawReading {
    RawReading {
        limitation_is_bad,
        packets_sent_total: sent_total,
        packets_lost_total: lost_total,
    }
}

#[test]
fn preset_bitrate_strictly_decreases_from_high_to_low() {
    assert!(
        preset_for(Tier::High).max_bitrate_bps > preset_for(Tier::Medium).max_bitrate_bps
            && preset_for(Tier::Medium).max_bitrate_bps > preset_for(Tier::Low).max_bitrate_bps
    );
}

#[test]
fn preset_scale_down_never_decreases_from_high_to_low() {
    assert!(
        preset_for(Tier::High).scale_down <= preset_for(Tier::Medium).scale_down
            && preset_for(Tier::Medium).scale_down <= preset_for(Tier::Low).scale_down
    );
}

#[test]
fn preset_framerate_never_increases_from_high_to_low() {
    assert!(
        preset_for(Tier::High).max_framerate >= preset_for(Tier::Medium).max_framerate
            && preset_for(Tier::Medium).max_framerate >= preset_for(Tier::Low).max_framerate
    );
}

#[test]
fn every_preset_caps_framerate_at_a_sane_positive_rate() {
    for tier in [Tier::High, Tier::Medium, Tier::Low] {
        let fps = preset_for(tier).max_framerate;
        assert!(
            fps > 0.0 && fps <= 60.0,
            "{tier:?} framerate {fps} outside (0, 60]"
        );
    }
}

#[test]
fn tier_for_maps_every_fixed_level_and_leaves_auto_as_none() {
    use screen_share_protocol::QualityLevel;
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
    assert_eq!(
        q.record_reading(reading(true, 100.0, 0.0)),
        None,
        "one bad reading alone shouldn't step"
    );
    assert_eq!(
        q.record_reading(reading(true, 200.0, 0.0)),
        Some(Tier::Medium)
    );
}

#[test]
fn steps_down_from_high_loss_ratio_even_without_a_limitation_reason() {
    let mut q = AdaptiveQuality::new();
    q.record_reading(reading(false, 0.0, 0.0));
    // 10% loss this interval (10/100), well above BAD_LOSS_RATIO.
    q.record_reading(reading(false, 100.0, 10.0));
    assert_eq!(
        q.record_reading(reading(false, 200.0, 20.0)),
        Some(Tier::Medium)
    );
}

#[test]
fn a_single_good_reading_between_bad_ones_resets_the_bad_streak() {
    let mut q = AdaptiveQuality::new();
    q.record_reading(reading(false, 0.0, 0.0));
    q.record_reading(reading(true, 100.0, 0.0));
    // Clean interval: 0% loss, no limitation — counts as Good, not just
    // "not bad", and must zero the bad streak so far.
    q.record_reading(reading(false, 200.0, 0.0));
    assert_eq!(
        q.record_reading(reading(true, 300.0, 0.0)),
        None,
        "the earlier bad streak must not have carried over"
    );
}

#[test]
fn steps_down_twice_reaches_low_not_past_it() {
    let mut q = AdaptiveQuality::new();
    q.record_reading(reading(false, 0.0, 0.0));
    q.record_reading(reading(true, 100.0, 0.0));
    assert_eq!(
        q.record_reading(reading(true, 200.0, 0.0)),
        Some(Tier::Medium)
    );
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
    assert_eq!(
        q.record_reading(reading(false, 400.0, 0.0)),
        None,
        "two good readings alone shouldn't step up yet"
    );
    assert_eq!(
        q.record_reading(reading(false, 500.0, 0.0)),
        Some(Tier::High)
    );
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
    assert_eq!(
        q.tier(),
        Tier::High,
        "sustained neutral readings should never move the tier"
    );
}
