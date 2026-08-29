//! SDP rewriting for the one thing the browser won't do on its own: make
//! the negotiated Opus stream music-grade.
//!
//! `getDisplayMedia` system audio is music, not a voice call, but Opus in
//! WebRTC negotiates a mono ~32 kbit/s voice profile by default. The
//! `a=fmtp` line for the Opus payload type has to be edited — before
//! `setLocalDescription` and before the SDP goes on the wire — to turn on
//! stereo and raise the bitrate ceiling. The sharer's live audio-quality
//! preset then picks the actual send rate at or below that ceiling via
//! `RTCRtpSender.setParameters` (no renegotiation); this module only
//! decides how high it is allowed to go.

/// Opus `maxaveragebitrate`, in bits per second, negotiated as the stream's
/// ceiling. 256 kbit/s is transparent stereo for music — past it Opus gains
/// nothing audible, and it stays well under what a small P2P mesh can move.
pub const OPUS_MAX_AVERAGE_BITRATE_BPS: u32 = 256_000;

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
    let opus_pts = opus_payload_types(sdp);
    if opus_pts.is_empty() {
        return sdp.to_string();
    }
    let pts_with_fmtp = payload_types_with_fmtp(sdp);

    let mut out: Vec<String> = Vec::with_capacity(sdp.split('\n').count() + opus_pts.len());
    for line in sdp.split('\n') {
        let (body, ending) = split_line_ending(line);

        if let Some(pt) = fmtp_payload_type(body) {
            if opus_pts.contains(&pt) {
                out.push(format!("{}{ending}", rewrite_fmtp_line(body, pt)));
                continue;
            }
        }

        out.push(line.to_string());

        if let Some(pt) = rtpmap_opus_payload_type(body) {
            if !pts_with_fmtp.contains(&pt) {
                out.push(format!("{}{ending}", synthesise_fmtp_line(pt)));
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

fn synthesise_fmtp_line(pt: u32) -> String {
    let params = forced_fmtp_keys()
        .into_iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join(";");
    format!("a=fmtp:{pt} {params}")
}

/// Merges the forced keys into an existing `a=fmtp:<pt> ...` line, keeping
/// any unrelated keys and their original order, overriding a forced key in
/// place if it's already there and appending it otherwise.
fn rewrite_fmtp_line(line: &str, pt: u32) -> String {
    let existing = line
        .strip_prefix(&format!("a=fmtp:{pt}"))
        .map(str::trim_start)
        .unwrap_or("");

    let forced = forced_fmtp_keys();
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
            tokens.push((forced_key.to_string(), forced_value));
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
#[path = "sdp_tests.rs"]
mod tests;
