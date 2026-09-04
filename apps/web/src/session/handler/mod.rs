#[cfg(feature = "hydrate")]
use crate::session::RoomMember;
#[cfg(feature = "hydrate")]
use crate::session::RoomState;
#[cfg(feature = "hydrate")]
use leptos::prelude::*;

#[cfg(feature = "hydrate")]
use leptos::task::spawn_local;
#[cfg(feature = "hydrate")]
use wasm_bindgen::JsCast;
#[cfg(feature = "hydrate")]
use web_sys::{MediaStream, RtcPeerConnectionIceEvent, RtcTrackEvent};

#[cfg(feature = "hydrate")]
use crate::client::seam::peer_link::PeerLink;
#[cfg(feature = "hydrate")]
use crate::client::webrtc::new_peer_connection;
#[cfg(feature = "hydrate")]
use crate::session::RoomSession;
#[cfg(feature = "hydrate")]
use screen_share_protocol::{ClientMessage, ServerMessage};

/// The JS event callbacks bound to one peer's `RTCPeerConnection`
/// (`ontrack` — only for a connection we receive on — plus
/// `onicecandidate` and `oniceconnectionstatechange`). Held in
/// `RoomSession::{outgoing,incoming}_callbacks` so they — and the
/// `RoomSession` clone one of them captures — drop when the connection is
/// removed or the room page unmounts, instead of being `Closure::forget`'d
/// and leaked for the life of the tab. Never read; the `Vec` just owns the
/// closures for exactly that long.
#[cfg(feature = "hydrate")]
pub(crate) struct PeerCallbacks(#[allow(dead_code)] Vec<Box<dyn std::any::Any>>);

#[cfg(all(feature = "hydrate", test))]
impl PeerCallbacks {
    /// An empty entry, for tests that only need something in the map.
    pub(crate) fn empty_for_test() -> Self {
        Self(Vec::new())
    }
}

/// Everything the `Joined` snapshot carries, bundled so `apply_joined_snapshot`
/// takes one argument for it instead of seven — same idea as `RoomState`.
#[cfg(feature = "hydrate")]
pub(crate) struct JoinedSnapshot {
    pub(crate) room_code: String,
    pub(crate) room_name: String,
    pub(crate) peer_id: String,
    pub(crate) members: Vec<screen_share_protocol::MemberInfo>,
    pub(crate) active_sharers: Vec<String>,
    pub(crate) watcher_info: Vec<screen_share_protocol::WatcherInfo>,
    pub(crate) latencies: Vec<screen_share_protocol::LatencyInfo>,
    pub(crate) turn: Option<screen_share_protocol::TurnCredentials>,
}

#[cfg(feature = "hydrate")]
pub(crate) fn apply_joined_snapshot(snapshot: JoinedSnapshot, signals: RoomState) {
    use std::collections::HashSet;

    use crate::client::storage::save_recent_room;
    use crate::features::profile::RecentRoom;

    let JoinedSnapshot {
        room_code,
        room_name,
        peer_id,
        members: joined_members,
        active_sharers,
        watcher_info,
        latencies,
        turn,
    } = snapshot;
    let RoomState {
        set_my_peer_id,
        set_members,
        set_room_name,
        set_authenticated,
        set_status,
        watchers_by_sharer,
        latency_by_peer,
        turn_credentials,
        ..
    } = signals;

    let sharer_set: HashSet<String> = active_sharers.into_iter().collect();
    let members: Vec<RoomMember> = joined_members
        .into_iter()
        .map(|m| RoomMember {
            sharing: sharer_set.contains(m.peer_id.as_str()),
            peer_id: m.peer_id.into(),
            nick: m.nick.into(),
            color: m.color.into(),
        })
        .collect();
    watchers_by_sharer.set(
        watcher_info
            .into_iter()
            .map(|w| {
                (
                    w.sharer_id.into(),
                    w.watchers.into_iter().map(String::from).collect(),
                )
            })
            .collect(),
    );
    latency_by_peer.set(
        latencies
            .into_iter()
            .map(|l| (l.peer_id.into(), l.ms))
            .collect(),
    );
    turn_credentials.set(turn);
    save_recent_room(RecentRoom {
        code: room_code,
        name: room_name.clone(),
    });
    set_my_peer_id.set(Some(peer_id));
    set_members.set(members);
    set_room_name.set(Some(room_name));
    set_authenticated.set(true);
    set_status.set("Conectado.".to_string());
}

// --- teardown helpers ---------------------------------------------------
//
// A (sharer, viewer) peer connection is torn down from several messages
// (`PeerLeft`, `PeerStoppedSharing`, `WatchStopped`) and from
// `media`/`watch`. These two helpers are the one place the
// close-and-forget sequence for each direction lives, so the order —
// stop the Auto poll, `close()` the `RTCPeerConnection`, then drop its
// retained callbacks — stays identical everywhere.

/// Tear down the outgoing (we-are-the-sharer → `peer_id`-is-the-viewer)
/// connection, if any, and stop that viewer's Auto-quality poll.
#[cfg(feature = "hydrate")]
pub(crate) fn teardown_outgoing(conn: &RoomSession, peer_id: &str) {
    super::quality::stop_auto_polling(conn, peer_id);
    if let Some(pc) = conn.outgoing.borrow_mut().remove(peer_id) {
        pc.close();
    }
    conn.outgoing_callbacks.borrow_mut().remove(peer_id);
}

/// Tear down the incoming (`peer_id`-is-the-sharer → we-are-the-viewer)
/// connection, if any.
#[cfg(feature = "hydrate")]
pub(crate) fn teardown_incoming(conn: &RoomSession, peer_id: &str) {
    if let Some(pc) = conn.incoming.borrow_mut().remove(peer_id) {
        pc.close();
    }
    conn.incoming_callbacks.borrow_mut().remove(peer_id);
}

/// Drop grid focus when the focused tile belongs to `peer_id` (or the
/// browser was showing that tile fullscreen), so focus mode never points
/// at a card that just disappeared.
#[cfg(feature = "hydrate")]
fn drop_focus_if_showing(expanded: RwSignal<Option<String>>, peer_id: &str) {
    let was_fullscreen = crate::features::room::media_controls::exit_fullscreen_if_showing(peer_id);
    expanded.update(|current| {
        if current.as_deref() == Some(peer_id) || was_fullscreen {
            *current = None;
        }
    });
}

// --- per-message handlers ---------------------------------------------------

/// Fan the `Joined` snapshot out into the room signals, then, if this is a
/// reconnect's rejoin, replay what this member was doing before the drop.
#[cfg(feature = "hydrate")]
fn on_joined(conn: &RoomSession, signals: RoomState, snapshot: JoinedSnapshot) {
    let present_peer_ids: Vec<String> = snapshot
        .members
        .iter()
        .map(|m| m.peer_id.to_string())
        .collect();
    apply_joined_snapshot(snapshot, signals);
    // If this `Joined` is a reconnect's rejoin, re-assert what we were
    // doing before the drop (sharing, watching); a no-op on a first join.
    super::reconnect::replay_intent_after_rejoin(conn, signals, &present_peer_ids);
}

/// Same device joined this room in another tab — this connection was
/// replaced. `room_exists` must be forced: whoever created the room never
/// went through `start_room_check`, so that signal was never populated and
/// the gate would stay stuck on "Verificando sala..." forever.
#[cfg(feature = "hydrate")]
fn on_kicked(conn: &RoomSession, signals: RoomState) {
    conn.expected_close.set(true);
    if let Some(ws) = conn.ws.borrow().as_ref() {
        ws.close();
    }
    signals.set_authenticated.set(false);
    signals.set_room_exists.set(Some(true));
    signals.set_status.set(
        "Você entrou nessa sala em outra aba ou janela — esta conexão foi encerrada.".to_string(),
    );
}

/// A co-member we are watching sent an SDP offer: open the incoming
/// connection, wire its callbacks, and answer. Ignores an unsolicited
/// offer (defence in depth for F07) so it never opens a connection — and
/// leaks host/srflx ICE candidates — for a screen the user did not choose
/// to watch.
#[cfg(feature = "hydrate")]
fn answer_offer(conn: RoomSession, signals: RoomState, from: String, sdp: String) {
    if !signals.watching.get_untracked().contains(&from) {
        return;
    }
    let connection_errors = signals.connection_errors;
    let turn_credentials = signals.turn_credentials;
    spawn_local(async move {
        let pc = match new_peer_connection(turn_credentials.get_untracked().as_ref()) {
            Ok(pc) => pc,
            Err(err) => {
                web_sys::console::error_2(
                    &wasm_bindgen::JsValue::from_str(
                        "new_peer_connection (answering an offer) failed:",
                    ),
                    &err,
                );
                return;
            }
        };
        conn.incoming.borrow_mut().insert(from.clone(), pc.clone());
        connection_errors.update(|errors| {
            errors.remove(&from);
        });

        let sharer_id = from.clone();
        let ontrack = wasm_bindgen::prelude::Closure::<dyn FnMut(RtcTrackEvent)>::new(
            move |event: RtcTrackEvent| {
                if let Ok(stream) = event.streams().get(0).dyn_into::<MediaStream>() {
                    // Fires once per track (video + tab audio = twice);
                    // `play_stream_in` is idempotent and swallows the
                    // resulting play/AbortError.
                    crate::session::media::play_stream_in(
                        &format!("video-{sharer_id}"),
                        &stream,
                        false,
                    );
                }
            },
        );
        pc.set_ontrack(Some(ontrack.as_ref().unchecked_ref()));

        let target_id = from.clone();
        let conn_for_ice = conn.clone();
        let onicecandidate =
            wasm_bindgen::prelude::Closure::<dyn FnMut(RtcPeerConnectionIceEvent)>::new(
                move |event: RtcPeerConnectionIceEvent| {
                    if let Some(candidate) = event.candidate() {
                        if let Some(ws) = conn_for_ice.ws.borrow().as_ref() {
                            ws.send(&ClientMessage::IceCandidate {
                                to: target_id.clone(),
                                stream_owner: target_id.clone(),
                                candidate: candidate.candidate(),
                                sdp_mid: candidate.sdp_mid(),
                                sdp_m_line_index: candidate.sdp_m_line_index(),
                            });
                        }
                    }
                },
            );
        pc.set_onicecandidate(Some(onicecandidate.as_ref().unchecked_ref()));

        let failed_peer_id = from.clone();
        let oniceconnectionstatechange = {
            let pc_for_state = pc.clone();
            wasm_bindgen::prelude::Closure::<dyn FnMut()>::new(move || {
                if pc_for_state.ice_connection_state() == web_sys::RtcIceConnectionState::Failed {
                    connection_errors.update(|errors| {
                        errors.insert(failed_peer_id.clone());
                    });
                }
            })
        };
        pc.set_oniceconnectionstatechange(Some(
            oniceconnectionstatechange.as_ref().unchecked_ref(),
        ));

        conn.incoming_callbacks.borrow_mut().insert(
            from.clone(),
            PeerCallbacks(vec![
                Box::new(ontrack),
                Box::new(onicecandidate),
                Box::new(oniceconnectionstatechange),
            ]),
        );

        match pc.answer(&sdp).await {
            Ok(answer_sdp) => {
                if let Some(ws) = conn.ws.borrow().as_ref() {
                    ws.send(&ClientMessage::Answer {
                        to: from.clone(),
                        sdp: answer_sdp,
                    });
                }
            }
            Err(err) => web_sys::console::error_2(
                &wasm_bindgen::JsValue::from_str("create_answer failed:"),
                &err,
            ),
        }
    });
}

/// The viewer's side of the negotiation: apply the sharer's answer, and,
/// for a viewer still on `Auto`, re-assert the encoding that
/// `setRemoteDescription` can drop.
#[cfg(feature = "hydrate")]
fn accept_answer_from(conn: RoomSession, signals: RoomState, from: String, sdp: String) {
    let Some(pc) = conn.outgoing.borrow().get(&from).cloned() else {
        return;
    };
    let video_mode = signals.video_mode;
    spawn_local(async move {
        if let Err(err) = pc.accept_answer(&sdp).await {
            web_sys::console::error_2(
                &wasm_bindgen::JsValue::from_str("accept_answer failed:"),
                &err,
            );
            return;
        }
        // `setRemoteDescription` re-derives the encoder config from the
        // negotiated SDP — where the answer's `x-google-start-bitrate`
        // finally reaches the encoder — and can drop the per-encoding
        // bitrate/scale/framerate set before the offer. Re-assert them for
        // a viewer still on `Auto`; one who picked a fixed tier already had
        // it applied by `QualityRequested` (which also stopped the poll),
        // so leave that alone.
        if super::quality::is_auto_polling(&conn, &from) {
            let _ = super::quality::apply_tier(&pc, super::quality::Tier::High).await;
            let _ = super::video_mode::apply_video_mode(&pc, video_mode.get_untracked()).await;
        }
    });
}

/// Route a relayed ICE candidate to the connection it belongs to —
/// `incoming` when the candidate's `stream_owner` is the sender itself,
/// `outgoing` otherwise.
#[cfg(feature = "hydrate")]
fn route_ice_candidate(
    conn: &RoomSession,
    from: String,
    stream_owner: String,
    candidate: String,
    sdp_mid: Option<String>,
    sdp_m_line_index: Option<u16>,
) {
    let pc = if stream_owner == from {
        conn.incoming.borrow().get(&from).cloned()
    } else {
        conn.outgoing.borrow().get(&from).cloned()
    };
    if let Some(pc) = pc {
        pc.add_ice_candidate(&candidate, sdp_mid, sdp_m_line_index);
    }
}

/// Attach every track of the local capture to `pc`. When the share
/// currently carries no audio, reserve an audio m-line up front so a later
/// "trocar fonte" can swap one in without the renegotiation this path
/// never does (see `reserve_audio_mline`).
#[cfg(feature = "hydrate")]
fn attach_local_tracks(pc: &web_sys::RtcPeerConnection, stream: &MediaStream) {
    let mut shares_audio = false;
    for track in stream.get_tracks().iter() {
        let track: web_sys::MediaStreamTrack = track.unchecked_into();
        if track.kind() == "audio" {
            shares_audio = true;
        }
        pc.add_track_0(&track, stream);
    }
    if !shares_audio {
        crate::client::webrtc::reserve_audio_mline(pc, stream);
    }
}

/// The status sentence for a payload-less `ServerMessage` that only needs
/// to surface a message. Only the seven unit error variants collapsed into
/// one arm of `build_message_handler` reach here.
#[cfg(feature = "hydrate")]
fn fixed_status_text(msg: &ServerMessage) -> &'static str {
    match msg {
        ServerMessage::AuthFailed => "Senha incorreta.",
        ServerMessage::RoomNotFound => "Sala não encontrada ou já foi encerrada.",
        ServerMessage::RoomFull => "Essa sala já está cheia (máximo de 10 pessoas).",
        ServerMessage::AlreadyInRoom => {
            "Esta conexão já está em uma sala. Recarregue a página para entrar em outra."
        }
        ServerMessage::ServerAtCapacity => {
            "O servidor está sem capacidade no momento. Tente novamente em alguns minutos."
        }
        ServerMessage::InvalidInput => {
            "Nick, nome da sala ou cor inválidos. Verifique e tente de novo."
        }
        ServerMessage::TooManyAttempts => {
            "Muitas tentativas de senha erradas. Aguarde um pouco antes de tentar de novo."
        }
        _ => unreachable!("fixed_status_text called with a non-error variant"),
    }
}

/// A co-member asked to watch our screen: open the outgoing connection,
/// attach the local tracks, establish the screen-tuned encoding, wire the
/// callbacks, and send the offer. Ignores the request unless we actually
/// have a local stream to offer (defence in depth for F07).
#[cfg(feature = "hydrate")]
fn offer_to_watcher(conn: RoomSession, signals: RoomState, from: String) {
    if !conn.sharing.borrow().is_sharing() {
        return;
    }
    let RoomState {
        my_peer_id,
        connection_errors,
        turn_credentials,
        audio_preset,
        video_mode,
        ..
    } = signals;
    spawn_local(async move {
        let pc = match new_peer_connection(turn_credentials.get_untracked().as_ref()) {
            Ok(pc) => pc,
            Err(err) => {
                web_sys::console::error_2(
                    &wasm_bindgen::JsValue::from_str(
                        "new_peer_connection (offering to a watcher) failed:",
                    ),
                    &err,
                );
                return;
            }
        };
        conn.outgoing.borrow_mut().insert(from.clone(), pc.clone());
        connection_errors.update(|errors| {
            errors.remove(&from);
        });

        if let Some(stream) = conn.sharing.borrow().stream() {
            attach_local_tracks(&pc, stream);
        }

        // Establish the screen-tuned encoding (bitrate/scale/fps ceiling,
        // then the sharer's video mode and audio preset) before the offer
        // is built, so every viewer connection starts from it even if that
        // viewer never touches the quality menu. A later explicit tier or
        // Auto poll re-runs `apply_tier`; `apply_video_mode` owns
        // degradation preference and is re-asserted after each of those.
        let _ = super::quality::apply_tier(&pc, super::quality::Tier::High).await;
        let _ = super::video_mode::apply_video_mode(&pc, video_mode.get_untracked()).await;
        let _ = super::audio::apply_audio_preset(&pc, audio_preset.get_untracked()).await;

        // `Auto` is the default and every card shows it selected, but
        // nothing sends a `SetQuality` for it — without this a plain
        // "assistir" would pin `High` forever and never actually adapt.
        // Start the poll here, where the connection exists (unlike a racing
        // `QualityRequested`); `AlreadyApplied` because the `apply_tier` +
        // `apply_video_mode` above just set the encoding — a redundant
        // re-apply here would race the offer built just below.
        super::quality::start_auto_polling(
            conn.clone(),
            from.clone(),
            super::quality::InitialTier::AlreadyApplied,
        );

        let target_id = from.clone();
        // Unlike the `Offer` branch: here the remote peer is the viewer,
        // not the stream owner. `stream_owner` must be my own peer_id, not
        // `target_id`, or the other side stores the candidate in the wrong
        // map (`outgoing` instead of `incoming`) and the connection never
        // closes its ICE.
        let stream_owner_id = my_peer_id
            .get_untracked()
            .unwrap_or_else(|| target_id.clone());
        let conn_for_ice = conn.clone();
        let onicecandidate =
            wasm_bindgen::prelude::Closure::<dyn FnMut(RtcPeerConnectionIceEvent)>::new(
                move |event: RtcPeerConnectionIceEvent| {
                    if let Some(candidate) = event.candidate() {
                        if let Some(ws) = conn_for_ice.ws.borrow().as_ref() {
                            ws.send(&ClientMessage::IceCandidate {
                                to: target_id.clone(),
                                stream_owner: stream_owner_id.clone(),
                                candidate: candidate.candidate(),
                                sdp_mid: candidate.sdp_mid(),
                                sdp_m_line_index: candidate.sdp_m_line_index(),
                            });
                        }
                    }
                },
            );
        pc.set_onicecandidate(Some(onicecandidate.as_ref().unchecked_ref()));

        let failed_viewer_id = from.clone();
        let oniceconnectionstatechange = {
            let pc_for_state = pc.clone();
            wasm_bindgen::prelude::Closure::<dyn FnMut()>::new(move || {
                if pc_for_state.ice_connection_state() == web_sys::RtcIceConnectionState::Failed {
                    connection_errors.update(|errors| {
                        errors.insert(failed_viewer_id.clone());
                    });
                }
            })
        };
        pc.set_oniceconnectionstatechange(Some(
            oniceconnectionstatechange.as_ref().unchecked_ref(),
        ));

        conn.outgoing_callbacks.borrow_mut().insert(
            from.clone(),
            PeerCallbacks(vec![
                Box::new(onicecandidate),
                Box::new(oniceconnectionstatechange),
            ]),
        );

        match pc.offer().await {
            Ok(sdp) => {
                if let Some(ws) = conn.ws.borrow().as_ref() {
                    ws.send(&ClientMessage::Offer { to: from, sdp });
                }
            }
            Err(err) => web_sys::console::error_2(
                &wasm_bindgen::JsValue::from_str("create_offer failed:"),
                &err,
            ),
        }
    });
}

/// A viewer chose a fixed quality tier (or switched back to `Auto`).
/// Always stop that viewer's running Auto poll first, then either pin the
/// requested tier or restart the poll from `High`.
#[cfg(feature = "hydrate")]
fn apply_quality_request(
    conn: RoomSession,
    signals: RoomState,
    from: String,
    quality: screen_share_protocol::QualityLevel,
) {
    super::quality::stop_auto_polling(&conn, &from);
    match super::quality::tier_for(quality) {
        Some(tier) => {
            if let Some(pc) = conn.outgoing.borrow().get(&from).cloned() {
                let video_mode = signals.video_mode;
                spawn_local(async move {
                    let _ = super::quality::apply_tier(&pc, tier).await;
                    // `apply_tier` round-trips the encoding params;
                    // re-assert the video mode's degradation preference on
                    // top so a quality change never silently reverts Motion
                    // mode.
                    let _ =
                        super::video_mode::apply_video_mode(&pc, video_mode.get_untracked()).await;
                });
            }
        }
        // A deliberate switch back to `Auto`: the sender may be pinned to a
        // lower tier right now, so re-apply `High`.
        None => super::quality::start_auto_polling(
            conn.clone(),
            from,
            super::quality::InitialTier::ResetToHigh,
        ),
    }
}

/// Build the `ServerMessage` dispatcher for one WebSocket session. Each
/// arm either updates a room signal directly (roster, status, latency) or
/// hands off to a `fn` above; the negotiation-heavy arms (`Offer`,
/// `Answer`, `WatchRequested`, `QualityRequested`) live entirely in those.
//
// Over the 100-line lint by ~30: a flat `match` with one arm per
// `ServerMessage` variant. Cognitive complexity is low (no nesting, no
// shared mutable flow between arms) and splitting the dispatch itself
// would only scatter it; the length is the variant count, not tangled
// logic.
#[allow(clippy::too_many_lines)]
#[cfg(feature = "hydrate")]
pub(crate) fn build_message_handler(
    conn: RoomSession,
    signals: RoomState,
) -> impl Fn(ServerMessage) + 'static {
    let RoomState {
        set_status,
        set_members,
        watching,
        expanded,
        watchers_by_sharer,
        latency_by_peer,
        ..
    } = signals;

    move |msg: ServerMessage| match msg {
        ServerMessage::Joined {
            peer_id,
            room,
            room_name,
            members: joined_members,
            active_sharers,
            watcher_info,
            latencies,
            turn,
        } => on_joined(
            &conn,
            signals,
            JoinedSnapshot {
                room_code: room.to_string(),
                room_name,
                peer_id: peer_id.to_string(),
                members: joined_members,
                active_sharers: active_sharers.into_iter().map(String::from).collect(),
                watcher_info,
                latencies,
                turn,
            },
        ),
        other @ (ServerMessage::AuthFailed
        | ServerMessage::RoomNotFound
        | ServerMessage::RoomFull
        | ServerMessage::AlreadyInRoom
        | ServerMessage::ServerAtCapacity
        | ServerMessage::InvalidInput
        | ServerMessage::TooManyAttempts) => set_status.set(fixed_status_text(&other).to_string()),
        ServerMessage::Kicked => on_kicked(&conn, signals),
        ServerMessage::PeerJoined {
            peer_id,
            nick,
            color,
        } => {
            let (peer_id, nick, color) = (peer_id.to_string(), nick.to_string(), color.to_string());
            crate::client::desktop_bridge::notify_desktop_member_joined(&nick);
            set_members.update(|members| {
                members.push(RoomMember {
                    peer_id,
                    nick,
                    color,
                    sharing: false,
                })
            });
        }
        ServerMessage::PeerLeft { peer_id } => {
            let peer_id = peer_id.to_string();
            set_members.update(|members| members.retain(|m| m.peer_id != peer_id));
            teardown_outgoing(&conn, &peer_id);
            teardown_incoming(&conn, &peer_id);
            drop_focus_if_showing(expanded, &peer_id);
        }
        ServerMessage::PeerStartedSharing { peer_id } => {
            let peer_id = peer_id.to_string();
            set_members.update(|members| {
                if let Some(m) = members.iter_mut().find(|m| m.peer_id == peer_id) {
                    m.sharing = true;
                }
            });
        }
        ServerMessage::PeerStoppedSharing { peer_id } => {
            let peer_id = peer_id.to_string();
            set_members.update(|members| {
                if let Some(m) = members.iter_mut().find(|m| m.peer_id == peer_id) {
                    m.sharing = false;
                }
            });
            watching.update(|w| {
                w.remove(&peer_id);
            });
            drop_focus_if_showing(expanded, &peer_id);
            watchers_by_sharer.update(|w| {
                w.remove(&peer_id);
            });
            teardown_incoming(&conn, &peer_id);
        }
        ServerMessage::WatchersChanged {
            sharer_id,
            watchers,
        } => {
            let sharer_id = sharer_id.to_string();
            let watchers: Vec<String> = watchers.into_iter().map(String::from).collect();
            watchers_by_sharer.update(|w| {
                w.insert(sharer_id, watchers);
            });
        }
        ServerMessage::Pong => {
            // `take()`, not `get()`: an unmatched `Pong` (e.g. one arriving
            // after a fresh `Ping` overwrote the timestamp) must not be
            // timed against the wrong send.
            if let Some(sent_at) = conn.last_ping_sent_at.take() {
                if let Some(rtt_ms) = super::latency::round_trip_ms_since(sent_at) {
                    if let Some(ws) = conn.ws.borrow().as_ref() {
                        ws.send(&ClientMessage::ReportLatency { ms: rtt_ms });
                    }
                }
            }
        }
        ServerMessage::PeerLatency { peer_id, ms } => {
            let peer_id = peer_id.to_string();
            latency_by_peer.update(|l| {
                l.insert(peer_id, ms);
            });
        }
        ServerMessage::Offer { from, sdp } => {
            answer_offer(conn.clone(), signals, from.to_string(), sdp)
        }
        ServerMessage::Answer { from, sdp } => {
            accept_answer_from(conn.clone(), signals, from.to_string(), sdp)
        }
        ServerMessage::IceCandidate {
            from,
            stream_owner,
            candidate,
            sdp_mid,
            sdp_m_line_index,
        } => route_ice_candidate(
            &conn,
            from.to_string(),
            stream_owner.to_string(),
            candidate,
            sdp_mid,
            sdp_m_line_index,
        ),
        ServerMessage::WatchRequested { from } => {
            offer_to_watcher(conn.clone(), signals, from.to_string())
        }
        ServerMessage::WatchStopped { from } => teardown_outgoing(&conn, &from),
        ServerMessage::QualityRequested { from, quality } => {
            apply_quality_request(conn.clone(), signals, from.to_string(), quality)
        }
    }
}

#[cfg(all(test, target_arch = "wasm32", feature = "hydrate"))]
#[path = "wasm_tests.rs"]
mod wasm_tests;
