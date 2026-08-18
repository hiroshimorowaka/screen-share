use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::IntoResponse;
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;

use super::protocol::{ClientMessage, ServerMessage};
use super::registry::Registry;

pub async fn ws_handler(State(registry): State<Registry>, ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, registry))
}

async fn handle_socket(socket: WebSocket, registry: Registry) {
    let (mut ws_sender, mut ws_receiver) = socket.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<ServerMessage>();

    let send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            let json = serde_json::to_string(&msg).unwrap();
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
            ClientMessage::CreateRoom => {
                let (code, id) = registry.create_room(tx.clone());
                let _ = tx.send(ServerMessage::RoomCreated { room: code.clone(), peer_id: id.clone() });
                room_code = Some(code);
                peer_id = Some(id);
            }
            ClientMessage::Join { room } => match registry.join_room(&room, tx.clone()) {
                Some((id, _host)) => {
                    let _ = tx.send(ServerMessage::Joined { peer_id: id.clone() });
                    room_code = Some(room);
                    peer_id = Some(id);
                }
                None => {
                    let _ = tx.send(ServerMessage::RoomNotFound);
                }
            },
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
            ClientMessage::IceCandidate { to, candidate, sdp_mid, sdp_m_line_index } => {
                if let (Some(room), Some(from)) = (&room_code, &peer_id) {
                    registry.relay(
                        room,
                        &to,
                        ServerMessage::IceCandidate { from: from.clone(), candidate, sdp_mid, sdp_m_line_index },
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
