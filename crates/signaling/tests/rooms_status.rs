use axum::routing::get;
use axum::Router;
use screen_share_protocol::{ClientMessage, RoomStatus, ServerMessage};
use screen_share_signaling::registry::Registry;
use screen_share_signaling::rooms_status::room_status_handler;
use screen_share_signaling::state::SignalingState;
use screen_share_signaling::ws::ws_handler;

async fn spawn_test_server() -> (String, String) {
    use screen_share_signaling::handshake::HandshakeConfig;

    let signaling_state = SignalingState {
        registry: Registry::new(),
        turn: None,
        handshake: HandshakeConfig::permissive(),
        room_status_limiter: screen_share_signaling::rooms_status::RoomStatusLimiter::new(),
    };
    let app = Router::new()
        .route("/ws", get(ws_handler))
        .route("/api/rooms/{code}", get(room_status_handler))
        .with_state(signaling_state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
        .await
        .unwrap();
    });

    (format!("ws://{addr}/ws"), format!("http://{addr}"))
}

#[tokio::test]
async fn room_status_confirms_an_existing_room_without_leaking_its_name_or_size() {
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
            // F06: the human-chosen name and the occupancy are never
            // handed to this unauthenticated endpoint.
            name: None,
            member_count: None,
            requires_password: Some(true),
        }
    );
}

#[tokio::test]
async fn room_status_rate_limits_a_client_that_polls_it_too_fast() {
    let (_ws_url, http_url) = spawn_test_server().await;
    let client = reqwest::Client::new();
    let url = format!("{http_url}/api/rooms/WHATEVER0");

    // The limiter budget is 30 / 10s; a 31st request inside the window
    // must be refused with 429.
    let mut got_429 = false;
    for _ in 0..40 {
        let status = client.get(&url).send().await.unwrap().status();
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            got_429 = true;
            break;
        }
    }
    assert!(
        got_429,
        "fast polling of the room-status endpoint must eventually hit 429"
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
