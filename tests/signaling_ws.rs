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

#[tokio::test]
async fn host_receives_peer_joined_and_viewer_receives_relayed_offer() {
    let url = spawn_test_server().await;

    let (mut host_ws, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
    host_ws
        .send(Message::Text(serde_json::to_string(&ClientMessage::CreateRoom).unwrap().into()))
        .await
        .unwrap();

    let created: ServerMessage = match host_ws.next().await.unwrap().unwrap() {
        Message::Text(text) => serde_json::from_str(&text).unwrap(),
        other => panic!("mensagem inesperada: {other:?}"),
    };
    let (room_code, host_id) = match created {
        ServerMessage::RoomCreated { room, peer_id } => (room, peer_id),
        other => panic!("esperava RoomCreated, recebeu {other:?}"),
    };

    let (mut viewer_ws, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
    viewer_ws
        .send(Message::Text(
            serde_json::to_string(&ClientMessage::Join { room: room_code.clone() }).unwrap().into(),
        ))
        .await
        .unwrap();

    let joined: ServerMessage = match viewer_ws.next().await.unwrap().unwrap() {
        Message::Text(text) => serde_json::from_str(&text).unwrap(),
        other => panic!("mensagem inesperada: {other:?}"),
    };
    let viewer_id = match joined {
        ServerMessage::Joined { peer_id } => peer_id,
        other => panic!("esperava Joined, recebeu {other:?}"),
    };

    let peer_joined: ServerMessage = match host_ws.next().await.unwrap().unwrap() {
        Message::Text(text) => serde_json::from_str(&text).unwrap(),
        other => panic!("mensagem inesperada: {other:?}"),
    };
    assert_eq!(peer_joined, ServerMessage::PeerJoined { peer_id: viewer_id.clone() });

    host_ws
        .send(Message::Text(
            serde_json::to_string(&ClientMessage::Offer { to: viewer_id.clone(), sdp: "test-sdp".to_string() })
                .unwrap()
                .into(),
        ))
        .await
        .unwrap();

    let offer: ServerMessage = match viewer_ws.next().await.unwrap().unwrap() {
        Message::Text(text) => serde_json::from_str(&text).unwrap(),
        other => panic!("mensagem inesperada: {other:?}"),
    };
    assert_eq!(offer, ServerMessage::Offer { from: host_id, sdp: "test-sdp".to_string() });
}
