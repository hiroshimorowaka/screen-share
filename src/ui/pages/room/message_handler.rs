#[cfg(feature = "hydrate")]
use super::connection::RoomSignals;
#[cfg(feature = "hydrate")]
use super::RoomMember;
#[cfg(feature = "hydrate")]
use leptos::prelude::*;

/// Everything the `Joined` snapshot carries, bundled so `apply_joined_snapshot`
/// takes one argument for it instead of seven — same idea as `RoomSignals`.
#[cfg(feature = "hydrate")]
pub(super) struct JoinedSnapshot {
    pub(super) room_code: String,
    pub(super) room_name: String,
    pub(super) peer_id: String,
    pub(super) members: Vec<crate::signaling::protocol::MemberInfo>,
    pub(super) active_sharers: Vec<String>,
    pub(super) watcher_info: Vec<crate::signaling::protocol::WatcherInfo>,
    pub(super) latencies: Vec<crate::signaling::protocol::LatencyInfo>,
    pub(super) turn: Option<crate::signaling::protocol::TurnCredentials>,
}

#[cfg(feature = "hydrate")]
pub(super) fn apply_joined_snapshot(snapshot: JoinedSnapshot, signals: RoomSignals) {
    use std::collections::HashSet;

    use crate::ui::client::storage::save_recent_room;
    use crate::ui::profile::RecentRoom;

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

#[cfg(feature = "hydrate")]
pub(super) fn build_message_handler(
    conn: super::connection::RoomConnection,
    signals: RoomSignals,
) -> impl Fn(crate::signaling::protocol::ServerMessage) + 'static {
    use leptos::task::spawn_local;
    use wasm_bindgen::JsCast;
    use web_sys::{MediaStream, RtcPeerConnectionIceEvent, RtcTrackEvent};

    use crate::signaling::protocol::{ClientMessage, ServerMessage};
    use crate::ui::client::webrtc::{
        accept_answer, add_ice_candidate, create_answer, create_offer, new_peer_connection,
    };

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
        }
        ServerMessage::AuthFailed => set_status.set("Senha incorreta.".to_string()),
        ServerMessage::RoomNotFound => {
            set_status.set("Sala não encontrada ou já foi encerrada.".to_string())
        }
        ServerMessage::RoomFull => {
            set_status.set("Essa sala já está cheia (máximo de 10 pessoas).".to_string())
        }
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
            crate::ui::client::webrtc::notify_desktop_member_joined(&nick);
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
            if let Some(pc) = conn.incoming.borrow_mut().remove(&peer_id) {
                pc.close();
            }
            let was_fullscreen = super::media_controls::exit_fullscreen_if_showing(&peer_id);
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
            let was_fullscreen = super::media_controls::exit_fullscreen_if_showing(&peer_id);
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
                            if let Some(document) = web_sys::window().and_then(|w| w.document()) {
                                if let Some(video_el) =
                                    document.get_element_by_id(&format!("video-{sharer_id}"))
                                {
                                    let video: web_sys::HtmlVideoElement =
                                        video_el.unchecked_into();
                                    video.set_src_object(Some(&stream));
                                    let _ = video.play();
                                }
                            }
                        }
                    },
                );
                pc.set_ontrack(Some(ontrack.as_ref().unchecked_ref()));
                ontrack.forget();

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
                onicecandidate.forget();

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
                oniceconnectionstatechange.forget();

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
                spawn_local(async move {
                    if let Err(err) = accept_answer(&pc, &sdp).await {
                        web_sys::console::error_2(
                            &wasm_bindgen::JsValue::from_str("accept_answer failed:"),
                            &err,
                        );
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
                    for track in stream.get_tracks().iter() {
                        let track: web_sys::MediaStreamTrack = track.unchecked_into();
                        pc.add_track_0(&track, stream);
                    }
                }

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
                onicecandidate.forget();

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
                oniceconnectionstatechange.forget();

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
        }
        ServerMessage::QualityRequested { from, quality } => {
            super::quality::stop_auto_polling(&conn, &from);
            match super::quality::tier_for(quality) {
                Some(tier) => {
                    if let Some(pc) = conn.outgoing.borrow().get(&from).cloned() {
                        spawn_local(async move {
                            let _ = super::quality::apply_tier(&pc, tier).await;
                        });
                    }
                }
                None => super::quality::start_auto_polling(conn.clone(), from),
            }
        }
    }
}
