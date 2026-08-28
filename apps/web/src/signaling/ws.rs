use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::IntoResponse;
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;

use super::protocol::{ClientMessage, ServerMessage};
use super::registry::{JoinError, JoinRequest, Registry};
use super::turn::TurnConfig;

/// The real client IP as Fly's edge proxy sees it — unlike `X-Forwarded-For`,
/// this header is set by Fly itself from the actual TCP connection, so a
/// client can't spoof it by sending its own value. Used only to scope the
/// wrong-password lockout per client (`registry::join_room`); falls back to
/// a constant so brute-force protection still applies (just room-wide, the
/// old behavior) when running outside Fly, e.g. local dev.
fn client_key(headers: &HeaderMap) -> String {
    headers
        .get("fly-client-ip")
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned)
        .unwrap_or_else(|| "unknown".to_string())
}

pub async fn ws_handler(
    State(registry): State<Registry>,
    State(turn): State<Option<TurnConfig>>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    let client_key = client_key(&headers);
    ws.on_upgrade(move |socket| handle_socket(socket, registry, turn, client_key))
}

async fn handle_socket(
    socket: WebSocket,
    registry: Registry,
    turn: Option<TurnConfig>,
    client_key: String,
) {
    let (mut ws_sender, mut ws_receiver) = socket.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<ServerMessage>();

    let send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            let json = serde_json::to_string(&msg)
                .expect("ServerMessage holds only primitives/strings, so it always serializes");
            if ws_sender.send(Message::Text(json.into())).await.is_err() {
                break;
            }
        }
    });

    let mut room_code: Option<String> = None;
    let mut peer_id: Option<String> = None;
    // Minted fresh per `Joined` rather than once per connection — cheap
    // (one HMAC), and keeps this the single place that decides what a
    // member's ICE config looks like.
    let mint_turn_credentials = || turn.as_ref().map(TurnConfig::mint_credentials);

    while let Some(Ok(msg)) = ws_receiver.next().await {
        let Message::Text(text) = msg else { continue };
        let Ok(client_msg) = serde_json::from_str::<ClientMessage>(&text) else {
            continue;
        };

        match client_msg {
            ClientMessage::CreateRoom {
                nick,
                password,
                room_name,
                color,
                device_id,
            } => {
                let (code, snapshot) =
                    registry.create_room(nick, color, room_name, password, device_id, tx.clone());
                let _ = tx.send(ServerMessage::Joined {
                    peer_id: snapshot.peer_id.clone(),
                    room: code.clone(),
                    room_name: snapshot.room_name,
                    members: snapshot.members,
                    active_sharers: snapshot.active_sharers,
                    watcher_info: snapshot.watcher_info,
                    latencies: snapshot.latencies,
                    turn: mint_turn_credentials(),
                });
                room_code = Some(code);
                peer_id = Some(snapshot.peer_id);
            }
            ClientMessage::JoinRoom {
                room,
                nick,
                password,
                color,
                device_id,
            } => {
                let request = JoinRequest {
                    nick,
                    color,
                    password,
                    device_id,
                    client_key: client_key.clone(),
                    sender: tx.clone(),
                };
                match registry.join_room(&room, request) {
                    Ok(snapshot) => {
                        let _ = tx.send(ServerMessage::Joined {
                            peer_id: snapshot.peer_id.clone(),
                            room: room.clone(),
                            room_name: snapshot.room_name,
                            members: snapshot.members,
                            active_sharers: snapshot.active_sharers,
                            watcher_info: snapshot.watcher_info,
                            latencies: snapshot.latencies,
                            turn: mint_turn_credentials(),
                        });
                        peer_id = Some(snapshot.peer_id);
                        room_code = Some(room);
                    }
                    Err(JoinError::NotFound) => {
                        let _ = tx.send(ServerMessage::RoomNotFound);
                    }
                    Err(JoinError::WrongPassword) => {
                        let _ = tx.send(ServerMessage::AuthFailed);
                    }
                    Err(JoinError::Full) => {
                        let _ = tx.send(ServerMessage::RoomFull);
                    }
                    Err(JoinError::TooManyAttempts) => {
                        let _ = tx.send(ServerMessage::TooManyAttempts);
                    }
                }
            }
            ClientMessage::StartShare => {
                if let (Some(room), Some(id)) = (&room_code, &peer_id) {
                    registry.start_share(room, id);
                }
            }
            ClientMessage::StopShare => {
                if let (Some(room), Some(id)) = (&room_code, &peer_id) {
                    registry.stop_share(room, id);
                }
            }
            ClientMessage::WatchShare { sharer_id } => {
                if let (Some(room), Some(from)) = (&room_code, &peer_id) {
                    registry.add_watcher(room, &sharer_id, from);
                }
            }
            ClientMessage::StopWatching { sharer_id } => {
                if let (Some(room), Some(from)) = (&room_code, &peer_id) {
                    registry.remove_watcher(room, &sharer_id, from);
                }
            }
            ClientMessage::Ping => {
                let _ = tx.send(ServerMessage::Pong);
            }
            ClientMessage::ReportLatency { ms } => {
                if let (Some(room), Some(id)) = (&room_code, &peer_id) {
                    registry.report_latency(room, id, ms);
                }
            }
            ClientMessage::Offer { to, sdp } => {
                if let (Some(room), Some(from)) = (&room_code, &peer_id) {
                    registry.relay(
                        room,
                        &to,
                        ServerMessage::Offer {
                            from: from.clone(),
                            sdp,
                        },
                    );
                }
            }
            ClientMessage::Answer { to, sdp } => {
                if let (Some(room), Some(from)) = (&room_code, &peer_id) {
                    registry.relay(
                        room,
                        &to,
                        ServerMessage::Answer {
                            from: from.clone(),
                            sdp,
                        },
                    );
                }
            }
            ClientMessage::IceCandidate {
                to,
                stream_owner,
                candidate,
                sdp_mid,
                sdp_m_line_index,
            } => {
                if let (Some(room), Some(from)) = (&room_code, &peer_id) {
                    registry.relay(
                        room,
                        &to,
                        ServerMessage::IceCandidate {
                            from: from.clone(),
                            stream_owner,
                            candidate,
                            sdp_mid,
                            sdp_m_line_index,
                        },
                    );
                }
            }
            ClientMessage::SetQuality { to, quality } => {
                if let (Some(room), Some(from)) = (&room_code, &peer_id) {
                    registry.relay(
                        room,
                        &to,
                        ServerMessage::QualityRequested {
                            from: from.clone(),
                            quality,
                        },
                    );
                }
            }
        }
    }

    if let (Some(room), Some(id)) = (room_code, peer_id) {
        registry.leave_room(&room, &id);
    }
    send_task.abort();
}
