//! Unit tests for `sdp`, split out of src/session/sdp.rs (refactor Phase 4).

use super::*;

/// A trimmed but realistic audio m-section as Chrome emits it, CRLF like
/// real SDP.
const OFFER_WITH_OPUS_FMTP: &str = "v=0\r\n\
o=- 1 2 IN IP4 127.0.0.1\r\n\
s=-\r\n\
t=0 0\r\n\
m=audio 9 UDP/TLS/RTP/SAVPF 111 63\r\n\
a=rtpmap:111 opus/48000/2\r\n\
a=fmtp:111 minptime=10;useinbandfec=1\r\n\
a=rtpmap:63 red/48000/2\r\n";

fn fmtp_line_for(sdp: &str, pt: &str) -> String {
    sdp.split("\r\n")
        .find(|line| line.starts_with(&format!("a=fmtp:{pt} ")))
        .unwrap_or_else(|| panic!("no a=fmtp:{pt} line in:\n{sdp}"))
        .to_string()
}

fn params_of(fmtp_line: &str) -> std::collections::HashMap<String, String> {
    fmtp_line
        .split_once(' ')
        .unwrap()
        .1
        .split(';')
        .map(|token| match token.split_once('=') {
            Some((k, v)) => (k.to_string(), v.to_string()),
            None => (token.to_string(), String::new()),
        })
        .collect()
}

#[test]
fn leaves_sdp_without_opus_untouched() {
    let sdp = "v=0\r\nm=video 9 UDP/TLS/RTP/SAVPF 96\r\na=rtpmap:96 VP8/90000\r\n";
    assert_eq!(tune_opus_for_music(sdp), sdp);
}

#[test]
fn forces_stereo_bitrate_and_gapless_flags_onto_the_opus_fmtp() {
    let out = tune_opus_for_music(OFFER_WITH_OPUS_FMTP);
    let params = params_of(&fmtp_line_for(&out, "111"));

    assert_eq!(params.get("stereo").map(String::as_str), Some("1"));
    assert_eq!(params.get("sprop-stereo").map(String::as_str), Some("1"));
    assert_eq!(
        params.get("maxaveragebitrate").map(String::as_str),
        Some(OPUS_MAX_AVERAGE_BITRATE_BPS.to_string().as_str())
    );
    assert_eq!(
        params.get("maxplaybackrate").map(String::as_str),
        Some("48000")
    );
    assert_eq!(params.get("usedtx").map(String::as_str), Some("0"));
    assert_eq!(params.get("cbr").map(String::as_str), Some("0"));
}

#[test]
fn preserves_unrelated_fmtp_keys() {
    let out = tune_opus_for_music(OFFER_WITH_OPUS_FMTP);
    let params = params_of(&fmtp_line_for(&out, "111"));
    assert_eq!(params.get("minptime").map(String::as_str), Some("10"));
    assert_eq!(params.get("useinbandfec").map(String::as_str), Some("1"));
}

#[test]
fn overrides_an_existing_forced_key_in_place_without_duplicating_it() {
    let sdp = "m=audio 9 UDP/TLS/RTP/SAVPF 111\r\n\
a=rtpmap:111 opus/48000/2\r\n\
a=fmtp:111 stereo=0;maxaveragebitrate=16000\r\n";
    let out = tune_opus_for_music(sdp);
    let fmtp = fmtp_line_for(&out, "111");

    assert_eq!(
        fmtp.matches("stereo=").count(),
        2,
        "stereo + sprop-stereo, no dupes: {fmtp}"
    );
    assert_eq!(fmtp.matches("maxaveragebitrate=").count(), 1, "{fmtp}");
    let params = params_of(&fmtp);
    assert_eq!(params.get("stereo").map(String::as_str), Some("1"));
    assert_eq!(
        params.get("maxaveragebitrate").map(String::as_str),
        Some(OPUS_MAX_AVERAGE_BITRATE_BPS.to_string().as_str())
    );
}

#[test]
fn synthesises_an_fmtp_line_when_opus_is_declared_without_one() {
    let sdp = "m=audio 9 UDP/TLS/RTP/SAVPF 111\r\n\
a=rtpmap:111 opus/48000/2\r\n\
a=rtcp-fb:111 transport-cc\r\n";
    let out = tune_opus_for_music(sdp);

    let lines: Vec<&str> = out.split("\r\n").collect();
    let rtpmap_idx = lines
        .iter()
        .position(|l| l.starts_with("a=rtpmap:111 "))
        .unwrap();
    assert_eq!(
        lines[rtpmap_idx + 1],
        "a=fmtp:111 stereo=1;sprop-stereo=1;\
maxaveragebitrate=256000;maxplaybackrate=48000;usedtx=0;cbr=0",
        "the fmtp line is inserted directly after the rtpmap it belongs to"
    );
}

#[test]
fn is_idempotent() {
    let once = tune_opus_for_music(OFFER_WITH_OPUS_FMTP);
    let twice = tune_opus_for_music(&once);
    assert_eq!(once, twice);
}

#[test]
fn preserves_crlf_endings_and_the_rest_of_the_sdp() {
    let out = tune_opus_for_music(OFFER_WITH_OPUS_FMTP);
    assert!(out.contains("\r\n"), "CRLF endings kept");
    assert!(!out.contains("\n\n"), "no blank lines introduced");
    assert!(out.starts_with("v=0\r\n"));
    assert!(out.contains("m=audio 9 UDP/TLS/RTP/SAVPF 111 63\r\n"));
    assert!(out.contains("a=rtpmap:63 red/48000/2"));
}

#[test]
fn handles_bare_newline_sdp_without_inventing_carriage_returns() {
    let sdp = "m=audio 9 UDP/TLS/RTP/SAVPF 111\na=rtpmap:111 opus/48000/2\n";
    let out = tune_opus_for_music(sdp);
    assert!(!out.contains('\r'));
    assert!(out.contains("\na=fmtp:111 stereo=1;"));
}

#[test]
fn matches_opus_codec_name_case_insensitively() {
    let sdp = "m=audio 9 RTP/AVP 111\r\na=rtpmap:111 OPUS/48000/2\r\n";
    let out = tune_opus_for_music(sdp);
    assert!(out.contains("a=fmtp:111 stereo=1;"));
}
