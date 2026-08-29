//! Unit tests for `video_mode`, split out of src/session/video_mode.rs
//! (refactor Phase 4).

use super::*;

#[test]
fn default_is_motion() {
    assert_eq!(VideoMode::default(), VideoMode::Motion);
}

#[test]
fn value_round_trips_through_from_value_for_every_mode() {
    for mode in VideoMode::ALL {
        assert_eq!(VideoMode::from_value(mode.value()), Some(mode));
    }
    assert_eq!(VideoMode::from_value("nonsense"), None);
}

#[test]
fn content_hint_is_a_valid_media_stream_track_hint() {
    // The two values the WebRTC spec defines for screen/video content.
    assert_eq!(VideoMode::Detail.content_hint(), "detail");
    assert_eq!(VideoMode::Motion.content_hint(), "motion");
}

#[test]
fn labels_and_hints_are_present_and_distinct() {
    let labels: Vec<&str> = VideoMode::ALL.iter().map(|m| m.label()).collect();
    let hints: Vec<&str> = VideoMode::ALL.iter().map(|m| m.hint()).collect();
    assert!(labels.iter().chain(&hints).all(|s| !s.is_empty()));
    assert_ne!(labels[0], labels[1]);
    assert_ne!(hints[0], hints[1]);
}
