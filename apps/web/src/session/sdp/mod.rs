//! SDP rewriting for two things the browser won't do well on its own:
//! music-grade Opus, and a sane *starting* video bitrate. Both edit
//! `a=fmtp` lines just before the description is set, and the sharer applies
//! them to *both* descriptions of each viewer connection — its local offer
//! (`create_offer`) and the answer it gets back (`accept_answer`) — because
//! Chrome reads a codec's `x-google-*` / Opus fmtp parameters, for the
//! direction it is *sending*, from the **remote** description, and strips
//! them from the answer it generates.
//!
//! **Opus.** `getDisplayMedia` system audio is music, not a voice call, but
//! Opus in WebRTC negotiates a mono ~32 kbit/s voice profile by default.
//! The `a=fmtp` line for the Opus payload type is edited to turn on stereo
//! and raise the bitrate ceiling. The sharer's live audio-quality preset
//! then picks the actual send rate at or below that ceiling via
//! `RTCRtpSender.setParameters` (no renegotiation); this module only
//! decides how high it is allowed to go.
//!
//! **Video start bitrate.** WebRTC's send-side bandwidth estimator opens
//! every new connection at a stock ~300 kbit/s and ramps up over 10–30 s,
//! so a viewer who just clicked "assistir" watches a smeared image slowly
//! sharpen. Screen content is mostly static and cheap to send, so the
//! video codecs' `a=fmtp` lines get Chrome's `x-google-start-bitrate`
//! (plus a matching min/max) to open near the top and let the estimator —
//! and `QualityLevel::Auto` — trim *down* only if the link can't hold it.
//! The hint only reaches the encoder via the sharer's remote description
//! (the answer), so munging just the offer — as this module first did — was
//! a no-op that left the ramp-up in place.

/// Opus `maxaveragebitrate`, in bits per second, negotiated as the stream's
/// ceiling. 256 kbit/s is transparent stereo for music — past it Opus gains
/// nothing audible, and it stays well under what a small P2P mesh can move.
pub const OPUS_MAX_AVERAGE_BITRATE_BPS: u32 = 256_000;

/// `x-google-start-bitrate`, in kbit/s: the rate the send-side estimator
/// assumes before it has measured the link. Deliberately *below*
/// [`VIDEO_MAX_BITRATE_KBPS`] so a weak connection isn't hit with a
/// full-ceiling burst on connect that it then has to claw back — but far
/// above Chrome's ~300 kbit/s stock start, so the picture opens sharp
/// instead of crawling up from a smear.
pub const VIDEO_START_BITRATE_KBPS: u32 = 2_500;
/// `x-google-min-bitrate`, in kbit/s — left at Chrome's own low floor so a
/// genuinely starved link is never wedged above what it can carry.
pub const VIDEO_MIN_BITRATE_KBPS: u32 = 300;
/// `x-google-max-bitrate`, in kbit/s. Matches the `High` tier ceiling in
/// [`crate::session::quality`] (`HIGH_MAX_BITRATE_BPS`, 4 Mbit/s); the live
/// per-tier `maxBitrate` from `apply_tier` still applies on top via
/// `setParameters`, so `Auto`/manual tier stepping keeps working — this
/// only stops a codec's low *default* max from capping the top tier.
pub const VIDEO_MAX_BITRATE_KBPS: u32 = 4_000;

/// Primary video codecs (`a=rtpmap:<pt> <name>/90000`) whose `a=fmtp` line
/// carries the `x-google-*` bitrate hints. Excludes `rtx`, `red`,
/// `ulpfec`/`flexfec` — retransmission/FEC payloads, not encoders.
const PRIMARY_VIDEO_CODECS: [&str; 5] = ["VP8", "VP9", "H264", "AV1", "AV1X"];

/// The Chrome-only `x-google-*` bitrate hints forced onto every primary
/// video codec's `a=fmtp` line — see the module docs for why.
fn forced_video_bitrate_keys() -> [(&'static str, String); 3] {
    [
        (
            "x-google-start-bitrate",
            VIDEO_START_BITRATE_KBPS.to_string(),
        ),
        ("x-google-min-bitrate", VIDEO_MIN_BITRATE_KBPS.to_string()),
        ("x-google-max-bitrate", VIDEO_MAX_BITRATE_KBPS.to_string()),
    ]
}

/// The `a=fmtp` keys this module forces on, in the order they're emitted
/// when a fresh `a=fmtp` line has to be synthesised. Existing keys not in
/// this list (e.g. `minptime`, `useinbandfec`) are preserved as-is.
fn forced_fmtp_keys() -> [(&'static str, String); 6] {
    [
        ("stereo", "1".to_string()),
        ("sprop-stereo", "1".to_string()),
        (
            "maxaveragebitrate",
            OPUS_MAX_AVERAGE_BITRATE_BPS.to_string(),
        ),
        ("maxplaybackrate", "48000".to_string()),
        // System audio is continuous — discontinuous transmission is a
        // voice-call optimisation that clips quiet musical passages, so
        // force it off. Bitrate stays variable (`cbr=0`): Opus VBR spends
        // fewer bits on simple passages and sounds better at a given
        // average than constant bitrate.
        ("usedtx", "0".to_string()),
        ("cbr", "0".to_string()),
    ]
}

/// Rewrites every Opus `a=fmtp` line in `sdp` for stereo, high-bitrate,
/// gapless audio, synthesising the line where a codec is declared without
/// one. Idempotent: re-running it produces the same output. If the SDP
/// declares no Opus codec it is returned unchanged. Line endings (`\r\n`
/// vs `\n`) are preserved per line.
pub fn tune_opus_for_music(sdp: &str) -> String {
    force_fmtp_keys(
        sdp,
        &opus_payload_types(sdp),
        rtpmap_opus_payload_type,
        &forced_fmtp_keys(),
    )
}

/// Adds Chrome's `x-google-start-bitrate` / `x-google-min-bitrate` /
/// `x-google-max-bitrate` to every primary video codec's `a=fmtp` line
/// (synthesising one where the codec has none, as VP8 usually does), so a
/// freshly-connected viewer starts near full quality instead of watching
/// the send-side estimator crawl up from its ~300 kbit/s default. Same
/// guarantees as [`tune_opus_for_music`]: idempotent, per-line endings and
/// every non-video line untouched, SDP with no video returned unchanged.
pub fn tune_video_start_bitrate(sdp: &str) -> String {
    force_fmtp_keys(
        sdp,
        &video_payload_types(sdp),
        rtpmap_video_payload_type,
        &forced_video_bitrate_keys(),
    )
}

/// Shared core of the two `tune_*` entry points: merges `forced` into the
/// `a=fmtp` line of every payload type in `target_pts`, synthesising one
/// right after the codec's `a=rtpmap` when it has none. `rtpmap_target_pt`
/// pulls a target payload type out of an `a=rtpmap` line body (and returns
/// `None` for anything else). Returns `sdp` untouched when `target_pts` is
/// empty; otherwise every other line is copied through verbatim, endings
/// and all.
fn force_fmtp_keys(
    sdp: &str,
    target_pts: &[u32],
    rtpmap_target_pt: impl Fn(&str) -> Option<u32>,
    forced: &[(&'static str, String)],
) -> String {
    if target_pts.is_empty() {
        return sdp.to_string();
    }
    let pts_with_fmtp = payload_types_with_fmtp(sdp);

    let mut out: Vec<String> = Vec::with_capacity(sdp.split('\n').count() + target_pts.len());
    for line in sdp.split('\n') {
        let (body, ending) = split_line_ending(line);

        if let Some(pt) = fmtp_payload_type(body) {
            if target_pts.contains(&pt) {
                out.push(format!("{}{ending}", rewrite_fmtp_line(body, pt, forced)));
                continue;
            }
        }

        out.push(line.to_string());

        if let Some(pt) = rtpmap_target_pt(body) {
            if !pts_with_fmtp.contains(&pt) {
                out.push(format!("{}{ending}", synthesise_fmtp_line(pt, forced)));
            }
        }
    }
    out.join("\n")
}

/// Splits a `str::split('\n')` fragment into its content and its original
/// line ending (`"\r"` for a CRLF source, `""` otherwise), so a
/// synthesised neighbouring line can match it.
fn split_line_ending(line: &str) -> (&str, &str) {
    match line.strip_suffix('\r') {
        Some(body) => (body, "\r"),
        None => (line, ""),
    }
}

/// Payload types declared as Opus (`a=rtpmap:<pt> opus/48000/2`) — the
/// rtpmap channel count is always 2 for Opus even when it ends up mono.
fn opus_payload_types(sdp: &str) -> Vec<u32> {
    sdp.split('\n')
        .filter_map(|line| rtpmap_opus_payload_type(split_line_ending(line).0))
        .collect()
}

fn payload_types_with_fmtp(sdp: &str) -> Vec<u32> {
    sdp.split('\n')
        .filter_map(|line| fmtp_payload_type(split_line_ending(line).0))
        .collect()
}

/// Payload types declared as a primary video codec
/// (`a=rtpmap:<pt> VP8|VP9|H264|AV1|AV1X/90000`), matched case-insensitively.
fn video_payload_types(sdp: &str) -> Vec<u32> {
    sdp.split('\n')
        .filter_map(|line| rtpmap_video_payload_type(split_line_ending(line).0))
        .collect()
}

/// `Some(pt)` if `line` is `a=rtpmap:<pt> <name>/90000` and `<name>` is one
/// of [`PRIMARY_VIDEO_CODECS`] — i.e. an encoder, not `rtx`/`red`/FEC.
fn rtpmap_video_payload_type(line: &str) -> Option<u32> {
    let rest = line.strip_prefix("a=rtpmap:")?;
    let (pt, descr) = rest.split_once(' ')?;
    let name = descr.split('/').next()?;
    PRIMARY_VIDEO_CODECS
        .iter()
        .any(|codec| codec.eq_ignore_ascii_case(name))
        .then(|| pt.parse().ok())
        .flatten()
}

/// `Some(pt)` if `line` is `a=rtpmap:<pt> opus/48000/2` (codec name matched
/// case-insensitively, as RFC 4566 allows).
fn rtpmap_opus_payload_type(line: &str) -> Option<u32> {
    let rest = line.strip_prefix("a=rtpmap:")?;
    let (pt, descr) = rest.split_once(' ')?;
    let mut parts = descr.split('/');
    let name = parts.next()?;
    if !name.eq_ignore_ascii_case("opus") {
        return None;
    }
    pt.parse().ok()
}

/// `Some(pt)` if `line` is `a=fmtp:<pt> ...`.
fn fmtp_payload_type(line: &str) -> Option<u32> {
    let rest = line.strip_prefix("a=fmtp:")?;
    let pt = rest.split(' ').next()?;
    pt.parse().ok()
}

fn synthesise_fmtp_line(pt: u32, forced: &[(&'static str, String)]) -> String {
    let params = forced
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join(";");
    format!("a=fmtp:{pt} {params}")
}

/// Merges the forced keys into an existing `a=fmtp:<pt> ...` line, keeping
/// any unrelated keys and their original order, overriding a forced key in
/// place if it's already there and appending it otherwise.
fn rewrite_fmtp_line(line: &str, pt: u32, forced: &[(&'static str, String)]) -> String {
    let existing = line
        .strip_prefix(&format!("a=fmtp:{pt}"))
        .map(str::trim_start)
        .unwrap_or("");

    let mut tokens: Vec<(String, String)> = Vec::new();
    for token in existing.split(';').filter(|t| !t.is_empty()) {
        let (key, value) = match token.split_once('=') {
            Some((key, value)) => (key.to_string(), value.to_string()),
            None => (token.to_string(), String::new()),
        };
        match forced
            .iter()
            .find(|(forced_key, _)| forced_key.eq_ignore_ascii_case(&key))
        {
            Some((_, forced_value)) => tokens.push((key, forced_value.clone())),
            None => tokens.push((key, value)),
        }
    }
    for (forced_key, forced_value) in forced {
        if !tokens
            .iter()
            .any(|(key, _)| key.eq_ignore_ascii_case(forced_key))
        {
            tokens.push((forced_key.to_string(), forced_value.clone()));
        }
    }

    let params = tokens
        .into_iter()
        .map(|(key, value)| {
            if value.is_empty() {
                key
            } else {
                format!("{key}={value}")
            }
        })
        .collect::<Vec<_>>()
        .join(";");
    format!("a=fmtp:{pt} {params}")
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
