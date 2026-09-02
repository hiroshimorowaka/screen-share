//! Unit tests for `audio`, split out of src/session/audio.rs (refactor
//! Phase 4).

use super::*;

const ALL: [AudioPreset; 3] = AudioPreset::ALL;

#[test]
fn default_is_balanced() {
    assert_eq!(AudioPreset::default(), AudioPreset::Balanced);
}

#[test]
fn bitrate_strictly_increases_from_voice_to_music() {
    assert!(
        AudioPreset::Voice.bitrate_bps() < AudioPreset::Balanced.bitrate_bps()
            && AudioPreset::Balanced.bitrate_bps() < AudioPreset::Music.bitrate_bps()
    );
}

#[test]
fn no_preset_exceeds_the_negotiated_opus_ceiling() {
    for preset in ALL {
        assert!(
            preset.bitrate_bps() <= screen_share_domain::sdp::OPUS_MAX_AVERAGE_BITRATE_BPS,
            "{preset:?} asks for more than the SDP ceiling"
        );
    }
}

#[test]
fn music_preset_uses_the_full_negotiated_ceiling() {
    assert_eq!(
        AudioPreset::Music.bitrate_bps(),
        screen_share_domain::sdp::OPUS_MAX_AVERAGE_BITRATE_BPS
    );
}

#[test]
fn value_round_trips_through_from_value_for_every_preset() {
    for preset in ALL {
        assert_eq!(AudioPreset::from_value(preset.value()), Some(preset));
    }
    assert_eq!(AudioPreset::from_value("hi-fi"), None);
}

#[test]
fn labels_hints_and_values_are_present_and_distinct() {
    let distinct = |pick: fn(AudioPreset) -> &'static str| {
        let seen: std::collections::HashSet<&str> = ALL.iter().map(|p| pick(*p)).collect();
        assert!(seen.iter().all(|s| !s.is_empty()));
        assert_eq!(seen.len(), ALL.len());
    };
    distinct(AudioPreset::label);
    distinct(AudioPreset::hint);
    distinct(AudioPreset::value);
}
