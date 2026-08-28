use axum::routing::get;
use axum::Router;
use screen_share_protocol::{ClientMessage, RoomStatus, ServerMessage};
use screen_share_signaling::registry::Registry;
use screen_share_signaling::rooms_status::room_status_handler;
use screen_share_signaling::state::SignalingState;
use screen_share_signaling::ws::ws_handler;

async fn spawn_test_server() -> (String, String) {
    let signaling_state = SignalingState {
        registry: Registry::new(),
        turn: None,
    };
    let app = Router::new()
        .route("/ws", get(ws_handler))
        .route("/api/rooms/{code}", get(room_status_handler))
        .with_state(signaling_state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app.into_make_service())
            .await
            .unwrap();
    });

    (format!("ws://{addr}/ws"), format!("http://{addr}"))
}

#[tokio::test]
async fn room_status_reports_existing_room_with_name_and_member_count() {
    use futures_util::{SinkExt, StreamExt};
    use tokio_tungstenite::tungstenite::Message;

    let (ws_url, http_url) = spawn_test_server().await;
    let (mut ws, _) = tokio_tungstenite::connect_async(&ws_url).await.unwrap();

    let create = ClientMessage::CreateRoom {
        nick: "Ana".to_string(),
        password: Some("senha123".to_string()),
        room_name: "Sala dos lindos".to_string(),
        color: "coral".to_string(),
        device_id: "device-ana".to_string(),
    };
    ws.send(Message::Text(
        serde_json::to_string(&create).unwrap().into(),
    ))
    .await
    .unwrap();

    // Bounded so a mutant that suppresses the `Joined` reply fails the
    // test instead of hanging it (matching the helpers in the other
    // signaling test files); 5s is orders of magnitude over the real
    // in-process round trip.
    let frame = tokio::time::timeout(std::time::Duration::from_secs(5), ws.next())
        .await
        .expect("timed out waiting for the Joined reply")
        .expect("websocket closed before the Joined reply")
        .unwrap();
    let room = match frame {
        Message::Text(text) => match serde_json::from_str::<ServerMessage>(&text).unwrap() {
            ServerMessage::Joined { room, .. } => room,
            other => panic!("esperava Joined, recebeu {other:?}"),
        },
        other => panic!("mensagem inesperada: {other:?}"),
    };

    let status: RoomStatus = reqwest::get(format!("{http_url}/api/rooms/{room}"))
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        status,
        RoomStatus {
            exists: true,
            name: Some("Sala dos lindos".to_string()),
            member_count: Some(1),
            requires_password: Some(true),
        }
    );
}

#[tokio::test]
async fn room_status_reports_missing_room_as_nonexistent() {
    let (_ws_url, http_url) = spawn_test_server().await;
    let status: RoomStatus = reqwest::get(format!("{http_url}/api/rooms/NOPE0000"))
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        status,
        RoomStatus {
            exists: false,
            name: None,
            member_count: None,
            requires_password: None
        }
    );
}
