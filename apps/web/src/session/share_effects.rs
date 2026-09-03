//! Two hydrate-only effect wirings `RoomPage` would otherwise own inline:
//! the desktop tray's quick-share auto-flow, and the trio of effects that
//! react to a share starting or stopping (the audio self-test, the
//! outgoing-mute toggle, and copying the invite link). Kept here, next to
//! the `ShareUi` signals they read and write, instead of buried in
//! `RoomPage`'s body.

use leptos::prelude::*;

use crate::session::share_ui::ShareUi;
use crate::session::RoomSession;

/// Whether a captured share stream ended up with an audio track — the one
/// signal the web side has for "this share carries audio", since the
/// desktop picker's audio choice never crosses back to the renderer.
#[cfg(feature = "hydrate")]
fn stream_has_audio_track(stream: &web_sys::MediaStream) -> bool {
    use wasm_bindgen::JsCast;

    stream
        .get_tracks()
        .iter()
        .filter_map(|entry| entry.dyn_into::<web_sys::MediaStreamTrack>().ok())
        .any(|track| track.kind() == "audio")
}

#[cfg(not(feature = "hydrate"))]
pub(crate) fn setup_quick_share_auto_flow(
    _conn: RoomSession,
    _room_code: String,
    _authenticated: ReadSignal<bool>,
    _share: ShareUi,
    _set_status: WriteSignal<String>,
    _my_peer_id: ReadSignal<Option<String>>,
    _expanded: RwSignal<Option<String>>,
) {
}

/// The desktop tray's quick-share flow: once the room-creation join
/// authenticates, start sharing immediately with no click, then hand the
/// invite link to the desktop shell as soon as the share goes live. A
/// no-op unless the URL carries the `quick_share` flag (see
/// `crate::quick_share`) — a plain browser tab never sets it.
///
/// Each half has its own "already done" latch — `authenticated` and
/// `share.is_sharing` can each change more than once over the page's
/// life, but this must only ever fire once.
#[cfg(feature = "hydrate")]
pub(crate) fn setup_quick_share_auto_flow(
    conn: RoomSession,
    room_code: String,
    authenticated: ReadSignal<bool>,
    share: ShareUi,
    set_status: WriteSignal<String>,
    my_peer_id: ReadSignal<Option<String>>,
    expanded: RwSignal<Option<String>>,
) {
    use crate::features::room::{build_invite_link, leave_room};
    use crate::session::media::start_sharing;

    if !crate::quick_share::requested() {
        return;
    }

    let auto_share_started = RwSignal::new(false);
    let auto_share_notified = RwSignal::new(false);
    let room_code_for_notify = room_code.clone();
    let conn_for_cancel = conn.clone();

    Effect::new(move |_| {
        if authenticated.get() && !auto_share_started.get_untracked() {
            auto_share_started.set(true);
            // Nobody's watching this hidden window to pick a screen a
            // second time — cancelling the picker here means leaving,
            // not sitting in the room unshared forever.
            let conn_for_cancel = conn_for_cancel.clone();
            let room_code_for_cancel = room_code.clone();
            start_sharing(
                conn.clone(),
                share.set_is_sharing,
                share.own_preview_hidden,
                set_status,
                my_peer_id,
                expanded,
                move || leave_room(&conn_for_cancel, &room_code_for_cancel, my_peer_id),
            );
        }
    });

    Effect::new(move |_| {
        if share.is_sharing.get() && !auto_share_notified.get_untracked() {
            auto_share_notified.set(true);
            if let Some(link) = build_invite_link(&room_code_for_notify) {
                crate::infra::webrtc::notify_desktop_share_ready(&link);
            }
        }
    });
}

#[cfg(not(feature = "hydrate"))]
pub(crate) fn setup_share_side_effects(
    _conn: RoomSession,
    _room_code: String,
    _share: ShareUi,
    _invite_copied: RwSignal<bool>,
) {
}

/// The three effects that follow a share of ours starting or stopping:
/// probe the captured stream for audible sound and warn if none came
/// through, apply the sharer's outgoing-mute toggle to the live tracks,
/// and copy the invite link the moment the share goes live (skipped for
/// the quick-share flow, which hands the link to the desktop shell
/// itself instead).
#[cfg(feature = "hydrate")]
pub(crate) fn setup_share_side_effects(
    conn: RoomSession,
    room_code: String,
    share: ShareUi,
    invite_copied: RwSignal<bool>,
) {
    use crate::features::room::copy_invite_link;

    let conn_for_probe = conn.clone();
    Effect::new(move |_| {
        // Re-run after a source switch (see `share_generation`).
        share.share_generation.track();
        if !share.is_sharing.get() {
            share.audio_warning.set(None);
            share.share_has_audio.set(false);
            return;
        }
        let Some(stream) = conn_for_probe.sharing.borrow().stream().cloned() else {
            return;
        };
        // A desktop share that opted out of audio and one whose loopback
        // capture failed both arrive here as a video-only stream,
        // indistinguishable from the renderer. Treat "the stream
        // actually carries an audio track" as the intent: no track
        // means audio simply wasn't part of this share (not a failure
        // to warn about); a silent track is still flagged.
        let has_audio_track = stream_has_audio_track(&stream);
        share.share_has_audio.set(has_audio_track);
        leptos::task::spawn_local(async move {
            let health =
                crate::session::audio_health::probe_share_audio(&stream, has_audio_track).await;
            share.audio_warning.set(health.warning());
        });
    });

    // Applying/clearing the outgoing audio mute. Also resets the toggle
    // when a share ends, so the next share starts un-muted.
    Effect::new(move |_| {
        let muted = share.audio_muted.get();
        if !share.is_sharing.get() {
            if muted {
                share.audio_muted.set(false);
            }
            return;
        }
        crate::session::audio::set_shared_audio_muted(&conn, muted);
    });

    // Copy the invite link the moment a share of ours goes live, so
    // there's something ready to paste — the quick-share flow already
    // does this via the desktop shell, so skip it there.
    let quick_share_active = crate::quick_share::requested();
    Effect::new(move |_| {
        if share.is_sharing.get() && !quick_share_active {
            copy_invite_link(&room_code, invite_copied);
        }
    });
}
