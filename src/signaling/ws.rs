use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::IntoResponse;
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;

use super::protocol::{ClientMessage, ServerMessage};
use super::registry::{JoinError, Registry};

pub async fn ws_handler(State(registry): State<Registry>, ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, registry))
}

async fn handle_socket(socket: WebSocket, registry: Registry) {
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

    while let Some(Ok(msg)) = ws_receiver.next().await {
        let Message::Text(text) = msg else { continue };
        let Ok(client_msg) = serde_json::from_str::<ClientMessage>(&text) else { continue };

        match client_msg {
            ClientMessage::CreateRoom { nick, password, room_name, color, device_id } => {
                let (code, snapshot) = registry.create_room(nick, color, room_name, password, device_id, tx.clone());
                let _ = tx.send(ServerMessage::Joined {
                    peer_id: snapshot.peer_id.clone(),
                    room: code.clone(),
                    room_name: snapshot.room_name,
                    members: snapshot.members,
                    active_sharers: snapshot.active_sharers,
                    watcher_info: snapshot.watcher_info,
                });
                room_code = Some(code);
                peer_id = Some(snapshot.peer_id);
            }
            ClientMessage::JoinRoom { room, nick, password, color, device_id } => {
                match registry.join_room(&room, nick, color, password, device_id, tx.clone()) {
                    Ok(snapshot) => {
                        let _ = tx.send(ServerMessage::Joined {
                            peer_id: snapshot.peer_id.clone(),
                            room: room.clone(),
                            room_name: snapshot.room_name,
                            members: snapshot.members,
                            active_sharers: snapshot.active_sharers,
                            watcher_info: snapshot.watcher_info,
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
            ClientMessage::Offer { to, sdp } => {
                if let (Some(room), Some(from)) = (&room_code, &peer_id) {
                    registry.relay(room, &to, ServerMessage::Offer { from: from.clone(), sdp });
                }
            }
            ClientMessage::Answer { to, sdp } => {
                if let (Some(room), Some(from)) = (&room_code, &peer_id) {
                    registry.relay(room, &to, ServerMessage::Answer { from: from.clone(), sdp });
                }
            }
            ClientMessage::IceCandidate { to, stream_owner, candidate, sdp_mid, sdp_m_line_index } => {
                if let (Some(room), Some(from)) = (&room_code, &peer_id) {
                    registry.relay(
                        room,
                        &to,
                        ServerMessage::IceCandidate { from: from.clone(), stream_owner, candidate, sdp_mid, sdp_m_line_index },
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
