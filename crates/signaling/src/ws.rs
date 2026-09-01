use std::collections::VecDeque;
use std::time::Duration;

use std::net::SocketAddr;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{ConnectInfo, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use futures_util::{SinkExt, StreamExt};
use tokio::time::Instant;

use super::handshake::HandshakeConfig;
use super::registry::{
    member_channel, ConnectionGuard, CreateRoomError, CreateRoomRequest, JoinError, JoinRequest,
    Registry,
};
use super::turn::TurnConfig;
use screen_share_protocol::{ClientMessage, ServerMessage};

/// Largest text frame the signaling socket accepts. Signaling payloads are
/// small — an SDP is a few KB, ICE candidates are trickled one per
/// message — so anything approaching this is abuse. Without the cap
/// axum-ws would buffer up to its ~64 MiB default before the frame is
/// even visible here.
const MAX_MESSAGE_BYTES: usize = 256 * 1024;

/// A connection is dropped if it sends nothing for this long. The client
/// pings every 5 s once joined (`apps/web` `session::latency`), so this is
/// ~18 missed pings: comfortably clear of a healthy connection, tight
/// enough to reap a slowloris socket that connects and then goes silent.
const IDLE_TIMEOUT: Duration = Duration::from_secs(90);

/// Sliding-window message-rate cap: at most [`MAX_MSGS_PER_WINDOW`]
/// client -> server messages per [`RATE_WINDOW`]. A join that starts
/// several watches at once produces a burst of offers/answers/ICE, so the
/// budget is generous; a flood loop still trips it and the connection is
/// closed.
const RATE_WINDOW: Duration = Duration::from_secs(10);
const MAX_MSGS_PER_WINDOW: usize = 300;

/// Drops timestamps older than [`RATE_WINDOW`] from `recent`, records
/// `now`, and reports whether the window is now over [`MAX_MSGS_PER_WINDOW`]
/// — i.e. whether this message pushes the connection past its rate budget.
fn over_rate_limit(recent: &mut VecDeque<Instant>, now: Instant) -> bool {
    while recent
        .front()
        .is_some_and(|stamp| now.duration_since(*stamp) >= RATE_WINDOW)
    {
        recent.pop_front();
    }
    recent.push_back(now);
    recent.len() > MAX_MSGS_PER_WINDOW
}

pub async fn ws_handler(
    State(registry): State<Registry>,
    State(turn): State<Option<TurnConfig>>,
    State(handshake): State<HandshakeConfig>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    // Defence in depth: refuse a cross-origin browser handshake before it
    // can drive any signaling. Not authentication — a non-browser client
    // sends no `Origin` — just a barrier for the "any website you visit"
    // attack surface.
    if !handshake.permits_origin(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    // Claim a connection slot before upgrading so a socket flood is
    // refused at the door rather than after allocating the task.
    let Some(connection) = registry.try_acquire_connection() else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    let client_key = handshake.client_key(&headers, peer);
    ws.max_message_size(MAX_MESSAGE_BYTES)
        .on_upgrade(move |socket| handle_socket(socket, registry, turn, client_key, connection))
}

async fn handle_socket(
    socket: WebSocket,
    registry: Registry,
    turn: Option<TurnConfig>,
    client_key: String,
    // Held for the whole task; dropping it frees the connection slot.
    _connection: ConnectionGuard,
) {
    let (mut ws_sender, mut ws_receiver) = socket.split();
    let (tx, mut rx) = member_channel();

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
    let mut recent_msgs: VecDeque<Instant> = VecDeque::new();
    // Minted fresh per `Joined` rather than once per connection — cheap
    // (one HMAC), and keeps this the single place that decides what a
    // member's ICE config looks like.
    let mint_turn_credentials = || turn.as_ref().map(TurnConfig::mint_credentials);

    loop {
        let received = tokio::time::timeout(IDLE_TIMEOUT, ws_receiver.next()).await;
        let msg = match received {
            // Idle timeout, stream ended, or transport error — all end the
            // connection the same way.
            Err(_) | Ok(None) | Ok(Some(Err(_))) => break,
            Ok(Some(Ok(msg))) => msg,
        };
        let Message::Text(text) = msg else { continue };

        if over_rate_limit(&mut recent_msgs, Instant::now()) {
            break;
        }

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
                if room_code.is_some() {
                    let _ = tx.try_send(ServerMessage::AlreadyInRoom);
                    continue;
                }
                match registry.create_room(CreateRoomRequest {
                    nick,
                    color,
                    room_name,
                    password,
                    device_id,
                    client_key: client_key.clone(),
                    sender: tx.clone(),
                }) {
                    Ok((code, snapshot)) => {
                        let _ = tx.try_send(ServerMessage::Joined {
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
                    Err(CreateRoomError::AtCapacity) => {
                        let _ = tx.try_send(ServerMessage::ServerAtCapacity);
                    }
                    Err(CreateRoomError::InvalidInput) => {
                        let _ = tx.try_send(ServerMessage::InvalidInput);
                    }
                }
            }
            ClientMessage::JoinRoom {
                room,
                nick,
                password,
                color,
                device_id,
            } => {
                if room_code.is_some() {
                    let _ = tx.try_send(ServerMessage::AlreadyInRoom);
                    continue;
                }
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
                        let _ = tx.try_send(ServerMessage::Joined {
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
                        let _ = tx.try_send(ServerMessage::RoomNotFound);
                    }
                    Err(JoinError::WrongPassword) => {
                        let _ = tx.try_send(ServerMessage::AuthFailed);
                    }
                    Err(JoinError::Full) => {
                        let _ = tx.try_send(ServerMessage::RoomFull);
                    }
                    Err(JoinError::TooManyAttempts) => {
                        let _ = tx.try_send(ServerMessage::TooManyAttempts);
                    }
                    Err(JoinError::InvalidInput) => {
                        let _ = tx.try_send(ServerMessage::InvalidInput);
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
                let _ = tx.try_send(ServerMessage::Pong);
            }
            ClientMessage::ReportLatency { ms } => {
                if let (Some(room), Some(id)) = (&room_code, &peer_id) {
                    registry.report_latency(room, id, ms);
                }
            }
            ClientMessage::Offer { to, sdp } => {
                if let (Some(room), Some(from)) = (&room_code, &peer_id) {
                    registry.relay_peer_signal(
                        room,
                        from,
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
                    registry.relay_peer_signal(
                        room,
                        from,
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
                    registry.relay_peer_signal(
                        room,
                        from,
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
                    registry.relay_peer_signal(
                        room,
                        from,
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

#[cfg(test)]
#[path = "ws_tests.rs"]
mod tests;
