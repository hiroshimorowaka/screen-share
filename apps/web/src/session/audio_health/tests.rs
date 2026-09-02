//! Unit tests for `audio_health`, split out of src/session/audio_health.rs
//! (refactor Phase 4).

use super::*;

#[test]
fn rms_of_silence_is_zero() {
    assert_eq!(rms(&[0.0; 256]), 0.0);
    assert_eq!(rms(&[]), 0.0);
}

#[test]
fn rms_of_a_full_scale_square_wave_is_one() {
    let block: Vec<f32> = (0..256)
        .map(|i| if i % 2 == 0 { 1.0 } else { -1.0 })
        .collect();
    assert!((rms(&block) - 1.0).abs() < 1e-6);
}

#[test]
fn a_quiet_tone_still_clears_the_silence_threshold() {
    // -60 dBFS sine — quiet, but real audio.
    let amp = 10f32.powf(-60.0 / 20.0);
    let block: Vec<f32> = (0..1024).map(|i| amp * (i as f32 * 0.2).sin()).collect();
    assert!(!is_effectively_silent(rms(&block)));
}

#[test]
fn interface_noise_floor_reads_as_silent() {
    // A dead capture still dithers at roughly -90 dBFS.
    let amp = 10f32.powf(-90.0 / 20.0);
    let block: Vec<f32> = (0..1024)
        .map(|i| amp * if i % 3 == 0 { 1.0 } else { -1.0 })
        .collect();
    assert!(is_effectively_silent(rms(&block)));
}

#[test]
fn classify_reports_not_shared_when_no_audio_was_expected() {
    // Even a captured, silent track: if audio wasn't asked for, it's not a
    // fault.
    assert_eq!(classify(false, false, false), AudioHealth::NotShared);
    assert_eq!(classify(false, true, false), AudioHealth::NotShared);
}

#[test]
fn classify_reports_capture_failed_when_expected_audio_has_no_track() {
    assert_eq!(classify(true, false, false), AudioHealth::CaptureFailed);
}

#[test]
fn classify_reports_silent_when_a_track_carried_no_sound() {
    assert_eq!(classify(true, true, false), AudioHealth::Silent);
}

#[test]
fn classify_reports_ok_when_a_track_carried_sound() {
    assert_eq!(classify(true, true, true), AudioHealth::Ok);
}

#[test]
fn only_the_two_fault_states_produce_a_warning() {
    assert!(AudioHealth::NotShared.warning().is_none());
    assert!(AudioHealth::Ok.warning().is_none());
    assert!(AudioHealth::CaptureFailed.warning().is_some());
    assert!(AudioHealth::Silent.warning().is_some());
}
