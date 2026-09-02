#[cfg(feature = "hydrate")]
use crate::session::RoomMember;
#[cfg(feature = "hydrate")]
use crate::session::RoomSignals;
#[cfg(feature = "hydrate")]
use leptos::prelude::*;

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
/// takes one argument for it instead of seven — same idea as `RoomSignals`.
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
pub(crate) fn apply_joined_snapshot(snapshot: JoinedSnapshot, signals: RoomSignals) {
    use std::collections::HashSet;

    use crate::features::profile::RecentRoom;
    use crate::infra::storage::save_recent_room;

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
    let RoomSignals {
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
            sharing: sharer_set.contains(&m.peer_id),
            peer_id: m.peer_id,
            nick: m.nick,
            color: m.color,
        })
        .collect();
    watchers_by_sharer.set(
        watcher_info
            .into_iter()
            .map(|w| (w.sharer_id, w.watchers))
            .collect(),
    );
    latency_by_peer.set(latencies.into_iter().map(|l| (l.peer_id, l.ms)).collect());
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

// 432 lines: one closure with a ~15-arm `match` over `ServerMessage`,
// each arm doing multi-step imperative signal mutation. Refactor step 3
// extracts one handler fn per arm and a shared `Peers::teardown`.
#[allow(clippy::too_many_lines)]
#[cfg(feature = "hydrate")]
pub(crate) fn build_message_handler(
    conn: crate::session::RoomSession,
    signals: RoomSignals,
) -> impl Fn(screen_share_protocol::ServerMessage) + 'static {
    use leptos::task::spawn_local;
    use wasm_bindgen::JsCast;
    use web_sys::{MediaStream, RtcPeerConnectionIceEvent, RtcTrackEvent};

    use crate::infra::webrtc::{
        accept_answer, add_ice_candidate, create_answer, create_offer, new_peer_connection,
    };
    use screen_share_protocol::{ClientMessage, ServerMessage};

    let RoomSignals {
        set_status,
        set_authenticated,
        set_members,
        my_peer_id,
        set_room_exists,
        watching,
        expanded,
        watchers_by_sharer,
        connection_errors,
        latency_by_peer,
        turn_credentials,
        audio_preset,
        video_mode,
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
        } => {
            let present_peer_ids: Vec<String> =
                joined_members.iter().map(|m| m.peer_id.clone()).collect();
            apply_joined_snapshot(
                JoinedSnapshot {
                    room_code: room,
                    room_name,
                    peer_id,
                    members: joined_members,
                    active_sharers,
                    watcher_info,
                    latencies,
                    turn,
                },
                signals,
            );
            // If this `Joined` is a reconnect's rejoin, re-assert what we
            // were doing before the drop (sharing, watching); a no-op on a
            // first join.
            super::reconnect::replay_intent_after_rejoin(&conn, signals, &present_peer_ids);
        }
        ServerMessage::AuthFailed => set_status.set("Senha incorreta.".to_string()),
        ServerMessage::RoomNotFound => {
            set_status.set("Sala não encontrada ou já foi encerrada.".to_string())
        }
        ServerMessage::RoomFull => {
            set_status.set("Essa sala já está cheia (máximo de 10 pessoas).".to_string())
        }
        ServerMessage::AlreadyInRoom => set_status.set(
            "Esta conexão já está em uma sala. Recarregue a página para entrar em outra."
                .to_string(),
        ),
        ServerMessage::ServerAtCapacity => set_status.set(
            "O servidor está sem capacidade no momento. Tente novamente em alguns minutos."
                .to_string(),
        ),
        ServerMessage::InvalidInput => set_status
            .set("Nick, nome da sala ou cor inválidos. Verifique e tente de novo.".to_string()),
        ServerMessage::TooManyAttempts => set_status.set(
            "Muitas tentativas de senha erradas. Aguarde um pouco antes de tentar de novo."
                .to_string(),
        ),
        ServerMessage::Kicked => {
            // Same device joined this room in another tab — this connection
            // was replaced. `room_exists` must be forced: whoever created
            // the room never went through `start_room_check`, so that
            // signal was never populated and the gate would stay stuck on
            // "Verificando sala..." forever.
            conn.expected_close.set(true);
            if let Some(ws) = conn.ws.borrow().as_ref() {
                ws.close();
            }
            set_authenticated.set(false);
            set_room_exists.set(Some(true));
            set_status.set(
                "Você entrou nessa sala em outra aba ou janela — esta conexão foi encerrada."
                    .to_string(),
            );
        }
        ServerMessage::PeerJoined {
            peer_id,
            nick,
            color,
        } => {
            crate::infra::webrtc::notify_desktop_member_joined(&nick);
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
            set_members.update(|members| members.retain(|m| m.peer_id != peer_id));
            super::quality::stop_auto_polling(&conn, &peer_id);
            if let Some(pc) = conn.outgoing.borrow_mut().remove(&peer_id) {
                pc.close();
            }
            conn.outgoing_callbacks.borrow_mut().remove(&peer_id);
            if let Some(pc) = conn.incoming.borrow_mut().remove(&peer_id) {
                pc.close();
            }
            conn.incoming_callbacks.borrow_mut().remove(&peer_id);
            let was_fullscreen =
                crate::features::room::media_controls::exit_fullscreen_if_showing(&peer_id);
            expanded.update(|current| {
                if current.as_deref() == Some(peer_id.as_str()) || was_fullscreen {
                    *current = None;
                }
            });
        }
        ServerMessage::PeerStartedSharing { peer_id } => {
            set_members.update(|members| {
                if let Some(m) = members.iter_mut().find(|m| m.peer_id == peer_id) {
                    m.sharing = true;
                }
            });
        }
        ServerMessage::PeerStoppedSharing { peer_id } => {
            set_members.update(|members| {
                if let Some(m) = members.iter_mut().find(|m| m.peer_id == peer_id) {
                    m.sharing = false;
                }
            });
            watching.update(|w| {
                w.remove(&peer_id);
            });
            let was_fullscreen =
                crate::features::room::media_controls::exit_fullscreen_if_showing(&peer_id);
            expanded.update(|current| {
                if current.as_deref() == Some(peer_id.as_str()) || was_fullscreen {
                    *current = None;
                }
            });
            watchers_by_sharer.update(|w| {
                w.remove(&peer_id);
            });
            if let Some(pc) = conn.incoming.borrow_mut().remove(&peer_id) {
                pc.close();
            }
            conn.incoming_callbacks.borrow_mut().remove(&peer_id);
        }
        ServerMessage::WatchersChanged {
            sharer_id,
            watchers,
        } => {
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
            latency_by_peer.update(|l| {
                l.insert(peer_id, ms);
            });
        }
        ServerMessage::Offer { from, sdp } => {
            // Defence in depth for F07: the relay only forwards an Offer
            // between peers already in a watch relationship, but ignore
            // one here too unless the user actually chose to watch `from`.
            // Never open a peer connection — and leak host/srflx ICE
            // candidates — for an unsolicited offer.
            if !watching.get_untracked().contains(&from) {
                return;
            }
            let conn = conn.clone();
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
                            // Fires once per track (video + tab audio =
                            // twice); `play_stream_in` is idempotent and
                            // swallows the resulting play/AbortError.
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
                        if pc_for_state.ice_connection_state()
                            == web_sys::RtcIceConnectionState::Failed
                        {
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

                match create_answer(&pc, &sdp).await {
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
        ServerMessage::Answer { from, sdp } => {
            if let Some(pc) = conn.outgoing.borrow().get(&from).cloned() {
                let conn = conn.clone();
                spawn_local(async move {
                    if let Err(err) = accept_answer(&pc, &sdp).await {
                        web_sys::console::error_2(
                            &wasm_bindgen::JsValue::from_str("accept_answer failed:"),
                            &err,
                        );
                        return;
                    }
                    // `setRemoteDescription` re-derives the encoder config
                    // from the negotiated SDP — where the answer's
                    // `x-google-start-bitrate` finally reaches the encoder —
                    // and can drop the per-encoding bitrate/scale/framerate
                    // set before the offer. Re-assert them for a viewer still
                    // on `Auto`; one who picked a fixed tier already had it
                    // applied by `QualityRequested` (which also stopped the
                    // poll), so leave that alone.
                    if super::quality::is_auto_polling(&conn, &from) {
                        let _ = super::quality::apply_tier(&pc, super::quality::Tier::High).await;
                        let _ =
                            super::video_mode::apply_video_mode(&pc, video_mode.get_untracked())
                                .await;
                    }
                });
            }
        }
        ServerMessage::IceCandidate {
            from,
            stream_owner,
            candidate,
            sdp_mid,
            sdp_m_line_index,
        } => {
            let pc = if stream_owner == from {
                conn.incoming.borrow().get(&from).cloned()
            } else {
                conn.outgoing.borrow().get(&from).cloned()
            };
            if let Some(pc) = pc {
                add_ice_candidate(&pc, &candidate, sdp_mid, sdp_m_line_index);
            }
        }
        ServerMessage::WatchRequested { from } => {
            // Defence in depth for F07 (mirrors the `Offer` branch): the
            // relay now only sends `WatchRequested` for a peer that is
            // actually sharing, but ignore one here too unless we have a
            // local stream to offer. Never open a peer connection — and
            // leak host/srflx ICE candidates — because a co-member asked
            // to watch a screen we aren't sharing.
            if conn.local_stream.borrow().is_none() {
                return;
            }
            let conn = conn.clone();
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

                if let Some(stream) = conn.local_stream.borrow().as_ref() {
                    let mut shares_audio = false;
                    for track in stream.get_tracks().iter() {
                        let track: web_sys::MediaStreamTrack = track.unchecked_into();
                        if track.kind() == "audio" {
                            shares_audio = true;
                        }
                        pc.add_track_0(&track, stream);
                    }
                    // A silent share can gain audio later via "trocar fonte";
                    // reserve the audio m-line now so `replace_outgoing_tracks`
                    // can swap that track in without a renegotiation this path
                    // never does (see `reserve_audio_mline`).
                    if !shares_audio {
                        crate::infra::webrtc::reserve_audio_mline(&pc, stream);
                    }
                }

                // Establish the screen-tuned encoding (bitrate/scale/fps
                // ceiling, then the sharer's video mode and audio preset)
                // before the offer is built, so every viewer connection
                // starts from it even if that viewer never touches the
                // quality menu. A later explicit tier or Auto poll re-runs
                // `apply_tier`; `apply_video_mode` owns degradation
                // preference and is re-asserted after each of those.
                let _ = super::quality::apply_tier(&pc, super::quality::Tier::High).await;
                let _ = super::video_mode::apply_video_mode(&pc, video_mode.get_untracked()).await;
                let _ = super::audio::apply_audio_preset(&pc, audio_preset.get_untracked()).await;

                // `Auto` is the default and every card shows it selected, but
                // nothing sends a `SetQuality` for it — without this a plain
                // "assistir" would pin `High` forever and never actually
                // adapt. Start the poll here, where the connection exists
                // (unlike a racing `QualityRequested`); `AlreadyApplied`
                // because the `apply_tier` + `apply_video_mode` above just
                // set the encoding — a redundant re-apply here would race
                // the offer built just below.
                super::quality::start_auto_polling(
                    conn.clone(),
                    from.clone(),
                    super::quality::InitialTier::AlreadyApplied,
                );

                let target_id = from.clone();
                // Unlike the `Offer` branch: here the remote peer is the
                // viewer, not the stream owner. `stream_owner` must be my
                // own peer_id, not `target_id`, or the other side stores the
                // candidate in the wrong map (`outgoing` instead of
                // `incoming`) and the connection never closes its ICE.
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
                        if pc_for_state.ice_connection_state()
                            == web_sys::RtcIceConnectionState::Failed
                        {
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

                match create_offer(&pc).await {
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
        ServerMessage::WatchStopped { from } => {
            super::quality::stop_auto_polling(&conn, &from);
            if let Some(pc) = conn.outgoing.borrow_mut().remove(&from) {
                pc.close();
            }
            conn.outgoing_callbacks.borrow_mut().remove(&from);
        }
        ServerMessage::QualityRequested { from, quality } => {
            super::quality::stop_auto_polling(&conn, &from);
            match super::quality::tier_for(quality) {
                Some(tier) => {
                    if let Some(pc) = conn.outgoing.borrow().get(&from).cloned() {
                        spawn_local(async move {
                            let _ = super::quality::apply_tier(&pc, tier).await;
                            // `apply_tier` round-trips the encoding params;
                            // re-assert the video mode's degradation
                            // preference on top so a quality change never
                            // silently reverts Motion mode.
                            let _ = super::video_mode::apply_video_mode(
                                &pc,
                                video_mode.get_untracked(),
                            )
                            .await;
                        });
                    }
                }
                // A deliberate switch back to `Auto`: the sender may be
                // pinned to a lower tier right now, so re-apply `High`.
                None => super::quality::start_auto_polling(
                    conn.clone(),
                    from,
                    super::quality::InitialTier::ResetToHigh,
                ),
            }
        }
    }
}

#[cfg(all(test, target_arch = "wasm32", feature = "hydrate"))]
#[path = "wasm_tests.rs"]
mod wasm_tests;
