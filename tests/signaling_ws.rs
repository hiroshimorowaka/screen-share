use futures_util::{SinkExt, StreamExt};
use screen_share::signaling::protocol::{ClientMessage, ServerMessage};
use tokio_tungstenite::tungstenite::Message;

async fn spawn_test_server() -> String {
    use axum::routing::get;
    use axum::Router;
    use screen_share::signaling::registry::Registry;
    use screen_share::signaling::ws::ws_handler;

    let registry = Registry::new();
    let app = Router::new().route("/ws", get(ws_handler)).with_state(registry);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app.into_make_service()).await.unwrap();
    });

    format!("ws://{addr}/ws")
}

async fn recv_json(ws: &mut (impl StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin)) -> ServerMessage {
    match ws.next().await.unwrap().unwrap() {
        Message::Text(text) => serde_json::from_str(&text).unwrap(),
        other => panic!("mensagem inesperada: {other:?}"),
    }
}

async fn send_json(
    ws: &mut (impl futures_util::Sink<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin),
    msg: &ClientMessage,
) {
    ws.send(Message::Text(serde_json::to_string(msg).unwrap().into())).await.unwrap();
}

#[tokio::test]
async fn create_room_then_join_with_wrong_and_right_password() {
    let url = spawn_test_server().await;

    let (mut creator_ws, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
    send_json(&mut creator_ws, &ClientMessage::CreateRoom {
        nick: "Ana".to_string(),
        password: "senha123".to_string(),
        room_name: "Sala da Ana".to_string(),
        color: "coral".to_string(),
    })
    .await;

    let room = match recv_json(&mut creator_ws).await {
        ServerMessage::Joined { room, members, .. } => {
            assert_eq!(members.len(), 1);
            room
        }
        other => panic!("esperava Joined, recebeu {other:?}"),
    };

    let (mut viewer_ws, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
    send_json(&mut viewer_ws, &ClientMessage::JoinRoom {
        room: room.clone(),
        nick: "Bia".to_string(),
        password: "senha-errada".to_string(),
        color: "sky".to_string(),
    })
    .await;
    assert_eq!(recv_json(&mut viewer_ws).await, ServerMessage::AuthFailed);

    send_json(&mut viewer_ws, &ClientMessage::JoinRoom {
        room: room.clone(),
        nick: "Bia".to_string(),
        password: "senha123".to_string(),
        color: "sky".to_string(),
    })
    .await;
    let viewer_id = match recv_json(&mut viewer_ws).await {
        ServerMessage::Joined { peer_id, members, .. } => {
            assert_eq!(members.len(), 2);
            peer_id
        }
        other => panic!("esperava Joined, recebeu {other:?}"),
    };

    assert_eq!(
        recv_json(&mut creator_ws).await,
        ServerMessage::PeerJoined { peer_id: viewer_id, nick: "Bia".to_string(), color: "sky".to_string() }
    );
}

#[tokio::test]
async fn start_share_broadcasts_and_offer_is_relayed() {
    let url = spawn_test_server().await;

    let (mut sharer_ws, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
    send_json(&mut sharer_ws, &ClientMessage::CreateRoom {
        nick: "Ana".to_string(),
        password: "senha123".to_string(),
        room_name: "Sala da Ana".to_string(),
        color: "coral".to_string(),
    })
    .await;
    let (room, sharer_id) = match recv_json(&mut sharer_ws).await {
        ServerMessage::Joined { room, peer_id, .. } => (room, peer_id),
        other => panic!("esperava Joined, recebeu {other:?}"),
    };

    let (mut viewer_ws, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
    send_json(&mut viewer_ws, &ClientMessage::JoinRoom {
        room: room.clone(),
        nick: "Bia".to_string(),
        password: "senha123".to_string(),
        color: "sky".to_string(),
    })
    .await;
    let viewer_id = match recv_json(&mut viewer_ws).await {
        ServerMessage::Joined { peer_id, .. } => peer_id,
        other => panic!("esperava Joined, recebeu {other:?}"),
    };
    recv_json(&mut sharer_ws).await; // drena o PeerJoined

    send_json(&mut sharer_ws, &ClientMessage::StartShare).await;
    assert_eq!(recv_json(&mut viewer_ws).await, ServerMessage::PeerStartedSharing { peer_id: sharer_id.clone() });

    send_json(&mut sharer_ws, &ClientMessage::Offer { to: viewer_id, sdp: "test-sdp".to_string() }).await;
    assert_eq!(recv_json(&mut viewer_ws).await, ServerMessage::Offer { from: sharer_id, sdp: "test-sdp".to_string() });
}

#[tokio::test]
async fn watch_share_notifies_the_sharer_and_broadcasts_watcher_count() {
    let url = spawn_test_server().await;

    let (mut sharer_ws, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
    send_json(&mut sharer_ws, &ClientMessage::CreateRoom {
        nick: "Ana".to_string(),
        password: "senha123".to_string(),
        room_name: "Sala da Ana".to_string(),
        color: "coral".to_string(),
    })
    .await;
    let (room, sharer_id) = match recv_json(&mut sharer_ws).await {
        ServerMessage::Joined { room, peer_id, .. } => (room, peer_id),
        other => panic!("esperava Joined, recebeu {other:?}"),
    };

    let (mut viewer_ws, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
    send_json(&mut viewer_ws, &ClientMessage::JoinRoom {
        room: room.clone(),
        nick: "Bia".to_string(),
        password: "senha123".to_string(),
        color: "sky".to_string(),
    })
    .await;
    let viewer_id = match recv_json(&mut viewer_ws).await {
        ServerMessage::Joined { peer_id, .. } => peer_id,
        other => panic!("esperava Joined, recebeu {other:?}"),
    };
    recv_json(&mut sharer_ws).await; // drena o PeerJoined

    send_json(&mut viewer_ws, &ClientMessage::WatchShare { sharer_id: sharer_id.clone() }).await;
    assert_eq!(recv_json(&mut sharer_ws).await, ServerMessage::WatchRequested { from: viewer_id.clone() });
    // A contagem nova vai pra sala inteira — quem compartilha e quem assiste
    // enxergam o mesmo "1 assistindo".
    assert_eq!(
        recv_json(&mut sharer_ws).await,
        ServerMessage::WatchersChanged { sharer_id: sharer_id.clone(), watchers: vec![viewer_id.clone()] }
    );
    assert_eq!(
        recv_json(&mut viewer_ws).await,
        ServerMessage::WatchersChanged { sharer_id: sharer_id.clone(), watchers: vec![viewer_id.clone()] }
    );

    send_json(&mut viewer_ws, &ClientMessage::StopWatching { sharer_id: sharer_id.clone() }).await;
    assert_eq!(recv_json(&mut sharer_ws).await, ServerMessage::WatchStopped { from: viewer_id.clone() });
    assert_eq!(
        recv_json(&mut sharer_ws).await,
        ServerMessage::WatchersChanged { sharer_id: sharer_id.clone(), watchers: vec![] }
    );
    assert_eq!(
        recv_json(&mut viewer_ws).await,
        ServerMessage::WatchersChanged { sharer_id, watchers: vec![] }
    );
}

#[tokio::test]
async fn room_not_found_for_unknown_code() {
    let url = spawn_test_server().await;
    let (mut ws, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
    send_json(&mut ws, &ClientMessage::JoinRoom { room: "NOPE0000".to_string(), nick: "Bia".to_string(), password: "x".to_string(), color: "sky".to_string() }).await;
    assert_eq!(recv_json(&mut ws).await, ServerMessage::RoomNotFound);
}
