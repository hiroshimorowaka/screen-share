use futures_util::{SinkExt, StreamExt};
use screen_share_protocol::{ClientMessage, QualityLevel, ServerMessage, MAX_MEMBERS};
use screen_share_signaling::registry::MAX_PASSWORD_ATTEMPTS;
use screen_share_signaling::turn::TurnConfig;
use tokio_tungstenite::tungstenite::Message;

use screen_share_signaling::handshake::HandshakeConfig;

async fn spawn_test_server() -> String {
    spawn_test_server_with_turn(None).await
}

async fn spawn_test_server_with_turn(turn: Option<TurnConfig>) -> String {
    let (url, _registry) = spawn_test_server_full(turn, HandshakeConfig::permissive()).await;
    url
}

/// Like [`spawn_test_server`] but also hands back the `Registry` the
/// server runs on, so a test can assert on server-side state (room count,
/// membership) that isn't observable over the socket alone.
async fn spawn_test_server_returning_registry(
    turn: Option<TurnConfig>,
) -> (String, screen_share_signaling::registry::Registry) {
    spawn_test_server_full(turn, HandshakeConfig::permissive()).await
}

async fn spawn_test_server_full(
    turn: Option<TurnConfig>,
    handshake: HandshakeConfig,
) -> (String, screen_share_signaling::registry::Registry) {
    use std::net::SocketAddr;

    use axum::routing::get;
    use axum::Router;
    use screen_share_signaling::registry::Registry;
    use screen_share_signaling::state::SignalingState;
    use screen_share_signaling::ws::ws_handler;

    let registry = Registry::new();
    let signaling_state = SignalingState {
        registry: registry.clone(),
        turn,
        handshake,
    };
    let app = Router::new()
        .route("/ws", get(ws_handler))
        .with_state(signaling_state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .unwrap();
    });

    (format!("ws://{addr}/ws"), registry)
}

/// Upper bound on how long any single server message may take to arrive.
/// A correct relay answers each of these tests near-instantly (everything
/// is in-process over loopback); anything slower means the relay dropped
/// the message. Bounding the wait makes that a test *failure* instead of
/// a hang — which also lets a mutation run see a suppressed broadcast as
/// a caught mutant rather than an inconclusive timeout.
const RECV_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

async fn recv_json(
    ws: &mut (impl StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin),
) -> ServerMessage {
    let frame = tokio::time::timeout(RECV_TIMEOUT, ws.next())
        .await
        .expect("timed out waiting for a server message")
        .expect("websocket closed while waiting for a server message")
        .unwrap();
    match frame {
        Message::Text(text) => serde_json::from_str(&text).unwrap(),
        other => panic!("mensagem inesperada: {other:?}"),
    }
}

async fn send_json(
    ws: &mut (impl futures_util::Sink<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin),
    msg: &ClientMessage,
) {
    ws.send(Message::Text(serde_json::to_string(msg).unwrap().into()))
        .await
        .unwrap();
}

#[tokio::test]
async fn create_room_then_join_with_wrong_and_right_password() {
    let url = spawn_test_server().await;

    let (mut creator_ws, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
    send_json(
        &mut creator_ws,
        &ClientMessage::CreateRoom {
            nick: "Ana".to_string(),
            password: Some("senha123".to_string()),
            room_name: "Sala da Ana".to_string(),
            color: "coral".to_string(),
            device_id: "device-ana".to_string(),
        },
    )
    .await;

    let room = match recv_json(&mut creator_ws).await {
        ServerMessage::Joined { room, members, .. } => {
            assert_eq!(members.len(), 1);
            room
        }
        other => panic!("esperava Joined, recebeu {other:?}"),
    };

    let (mut viewer_ws, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
    send_json(
        &mut viewer_ws,
        &ClientMessage::JoinRoom {
            room: room.clone(),
            nick: "Bia".to_string(),
            password: Some("senha-errada".to_string()),
            color: "sky".to_string(),
            device_id: "device-bia".to_string(),
        },
    )
    .await;
    assert_eq!(recv_json(&mut viewer_ws).await, ServerMessage::AuthFailed);

    send_json(
        &mut viewer_ws,
        &ClientMessage::JoinRoom {
            room: room.clone(),
            nick: "Bia".to_string(),
            password: Some("senha123".to_string()),
            color: "sky".to_string(),
            device_id: "device-bia".to_string(),
        },
    )
    .await;
    let viewer_id = match recv_json(&mut viewer_ws).await {
        ServerMessage::Joined {
            peer_id, members, ..
        } => {
            assert_eq!(members.len(), 2);
            peer_id
        }
        other => panic!("esperava Joined, recebeu {other:?}"),
    };

    assert_eq!(
        recv_json(&mut creator_ws).await,
        ServerMessage::PeerJoined {
            peer_id: viewer_id,
            nick: "Bia".to_string(),
            color: "sky".to_string()
        }
    );
}

#[tokio::test]
async fn start_share_broadcasts_and_offer_is_relayed() {
    let url = spawn_test_server().await;

    let (mut sharer_ws, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
    send_json(
        &mut sharer_ws,
        &ClientMessage::CreateRoom {
            nick: "Ana".to_string(),
            password: Some("senha123".to_string()),
            room_name: "Sala da Ana".to_string(),
            color: "coral".to_string(),
            device_id: "device-ana".to_string(),
        },
    )
    .await;
    let (room, sharer_id) = match recv_json(&mut sharer_ws).await {
        ServerMessage::Joined { room, peer_id, .. } => (room, peer_id),
        other => panic!("esperava Joined, recebeu {other:?}"),
    };

    let (mut viewer_ws, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
    send_json(
        &mut viewer_ws,
        &ClientMessage::JoinRoom {
            room: room.clone(),
            nick: "Bia".to_string(),
            password: Some("senha123".to_string()),
            color: "sky".to_string(),
            device_id: "device-bia".to_string(),
        },
    )
    .await;
    let viewer_id = match recv_json(&mut viewer_ws).await {
        ServerMessage::Joined { peer_id, .. } => peer_id,
        other => panic!("esperava Joined, recebeu {other:?}"),
    };
    recv_json(&mut sharer_ws).await; // drena o PeerJoined

    send_json(&mut sharer_ws, &ClientMessage::StartShare).await;
    assert_eq!(
        recv_json(&mut viewer_ws).await,
        ServerMessage::PeerStartedSharing {
            peer_id: sharer_id.clone()
        }
    );

    send_json(
        &mut sharer_ws,
        &ClientMessage::Offer {
            to: viewer_id,
            sdp: "test-sdp".to_string(),
        },
    )
    .await;
    assert_eq!(
        recv_json(&mut viewer_ws).await,
        ServerMessage::Offer {
            from: sharer_id,
            sdp: "test-sdp".to_string()
        }
    );
}

#[tokio::test]
async fn watch_share_notifies_the_sharer_and_broadcasts_watcher_count() {
    let url = spawn_test_server().await;

    let (mut sharer_ws, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
    send_json(
        &mut sharer_ws,
        &ClientMessage::CreateRoom {
            nick: "Ana".to_string(),
            password: Some("senha123".to_string()),
            room_name: "Sala da Ana".to_string(),
            color: "coral".to_string(),
            device_id: "device-ana".to_string(),
        },
    )
    .await;
    let (room, sharer_id) = match recv_json(&mut sharer_ws).await {
        ServerMessage::Joined { room, peer_id, .. } => (room, peer_id),
        other => panic!("esperava Joined, recebeu {other:?}"),
    };

    let (mut viewer_ws, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
    send_json(
        &mut viewer_ws,
        &ClientMessage::JoinRoom {
            room: room.clone(),
            nick: "Bia".to_string(),
            password: Some("senha123".to_string()),
            color: "sky".to_string(),
            device_id: "device-bia".to_string(),
        },
    )
    .await;
    let viewer_id = match recv_json(&mut viewer_ws).await {
        ServerMessage::Joined { peer_id, .. } => peer_id,
        other => panic!("esperava Joined, recebeu {other:?}"),
    };
    recv_json(&mut sharer_ws).await; // drena o PeerJoined

    send_json(
        &mut viewer_ws,
        &ClientMessage::WatchShare {
            sharer_id: sharer_id.clone(),
        },
    )
    .await;
    assert_eq!(
        recv_json(&mut sharer_ws).await,
        ServerMessage::WatchRequested {
            from: viewer_id.clone()
        }
    );
    assert_eq!(
        recv_json(&mut sharer_ws).await,
        ServerMessage::WatchersChanged {
            sharer_id: sharer_id.clone(),
            watchers: vec![viewer_id.clone()]
        }
    );
    assert_eq!(
        recv_json(&mut viewer_ws).await,
        ServerMessage::WatchersChanged {
            sharer_id: sharer_id.clone(),
            watchers: vec![viewer_id.clone()]
        }
    );

    send_json(
        &mut viewer_ws,
        &ClientMessage::StopWatching {
            sharer_id: sharer_id.clone(),
        },
    )
    .await;
    assert_eq!(
        recv_json(&mut sharer_ws).await,
        ServerMessage::WatchStopped {
            from: viewer_id.clone()
        }
    );
    assert_eq!(
        recv_json(&mut sharer_ws).await,
        ServerMessage::WatchersChanged {
            sharer_id: sharer_id.clone(),
            watchers: vec![]
        }
    );
    assert_eq!(
        recv_json(&mut viewer_ws).await,
        ServerMessage::WatchersChanged {
            sharer_id,
            watchers: vec![]
        }
    );
}

#[tokio::test]
async fn joining_from_the_same_device_kicks_the_previous_connection() {
    let url = spawn_test_server().await;

    let (mut creator_ws, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
    send_json(
        &mut creator_ws,
        &ClientMessage::CreateRoom {
            nick: "Ana".to_string(),
            password: Some("senha123".to_string()),
            room_name: "Sala da Ana".to_string(),
            color: "coral".to_string(),
            device_id: "device-ana".to_string(),
        },
    )
    .await;
    let (room, old_peer_id) = match recv_json(&mut creator_ws).await {
        ServerMessage::Joined { room, peer_id, .. } => (room, peer_id),
        other => panic!("esperava Joined, recebeu {other:?}"),
    };

    let (mut viewer_ws, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
    send_json(
        &mut viewer_ws,
        &ClientMessage::JoinRoom {
            room: room.clone(),
            nick: "Bia".to_string(),
            password: Some("senha123".to_string()),
            color: "sky".to_string(),
            device_id: "device-bia".to_string(),
        },
    )
    .await;
    recv_json(&mut viewer_ws).await; // drena o Joined da própria Bia
    recv_json(&mut creator_ws).await; // drena o PeerJoined da Bia

    let (mut second_ana_ws, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
    send_json(
        &mut second_ana_ws,
        &ClientMessage::JoinRoom {
            room: room.clone(),
            nick: "AnaCelular".to_string(),
            password: Some("senha123".to_string()),
            color: "coral".to_string(),
            device_id: "device-ana".to_string(),
        },
    )
    .await;

    assert_eq!(recv_json(&mut creator_ws).await, ServerMessage::Kicked);

    assert_eq!(
        recv_json(&mut viewer_ws).await,
        ServerMessage::PeerLeft {
            peer_id: old_peer_id
        }
    );
    let new_peer_id = match recv_json(&mut viewer_ws).await {
        ServerMessage::PeerJoined {
            peer_id,
            nick,
            color,
        } => {
            assert_eq!(nick, "AnaCelular");
            assert_eq!(color, "coral");
            peer_id
        }
        other => panic!("esperava PeerJoined, recebeu {other:?}"),
    };

    match recv_json(&mut second_ana_ws).await {
        ServerMessage::Joined {
            peer_id, members, ..
        } => {
            assert_eq!(peer_id, new_peer_id);
            assert_eq!(members.len(), 2);
        }
        other => panic!("esperava Joined, recebeu {other:?}"),
    }
}

#[tokio::test]
async fn room_not_found_for_unknown_code() {
    let url = spawn_test_server().await;
    let (mut ws, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
    send_json(
        &mut ws,
        &ClientMessage::JoinRoom {
            room: "NOPE0000".to_string(),
            nick: "Bia".to_string(),
            password: Some("x".to_string()),
            color: "sky".to_string(),
            device_id: "device-bia".to_string(),
        },
    )
    .await;
    assert_eq!(recv_json(&mut ws).await, ServerMessage::RoomNotFound);
}

#[tokio::test]
async fn joined_carries_turn_credentials_when_the_deployment_has_turn_configured() {
    let turn = TurnConfig::from_vars(
        Some("s3cr3t".to_string()),
        Some("turn:relay.example:3478".to_string()),
    );
    let url = spawn_test_server_with_turn(turn).await;

    let (mut ws, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
    send_json(
        &mut ws,
        &ClientMessage::CreateRoom {
            nick: "Ana".to_string(),
            password: None,
            room_name: "Sala da Ana".to_string(),
            color: "coral".to_string(),
            device_id: "device-ana".to_string(),
        },
    )
    .await;

    match recv_json(&mut ws).await {
        ServerMessage::Joined { turn, .. } => {
            let creds =
                turn.expect("a TURN-configured deployment must hand the client credentials");
            assert_eq!(creds.urls, vec!["turn:relay.example:3478".to_string()]);
            assert!(!creds.username.is_empty());
            assert!(!creds.password.is_empty());
        }
        other => panic!("esperava Joined, recebeu {other:?}"),
    }
}

/// One sharer + one viewer, then every remaining client→server message
/// the relay handles that the other tests don't already assert: the
/// `Answer` / `IceCandidate` / `SetQuality` relays, the `Ping`→`Pong`
/// echo, the `ReportLatency` broadcast, and `StopShare`.
#[tokio::test]
async fn relays_the_remaining_peer_to_peer_message_types() {
    let url = spawn_test_server().await;

    let (mut sharer_ws, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
    send_json(
        &mut sharer_ws,
        &ClientMessage::CreateRoom {
            nick: "Ana".to_string(),
            password: None,
            room_name: "Sala".to_string(),
            color: "coral".to_string(),
            device_id: "device-ana".to_string(),
        },
    )
    .await;
    let (room, sharer_id) = match recv_json(&mut sharer_ws).await {
        ServerMessage::Joined { room, peer_id, .. } => (room, peer_id),
        other => panic!("esperava Joined, recebeu {other:?}"),
    };

    let (mut viewer_ws, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
    send_json(
        &mut viewer_ws,
        &ClientMessage::JoinRoom {
            room: room.clone(),
            nick: "Bia".to_string(),
            password: None,
            color: "sky".to_string(),
            device_id: "device-bia".to_string(),
        },
    )
    .await;
    let viewer_id = match recv_json(&mut viewer_ws).await {
        ServerMessage::Joined { peer_id, .. } => peer_id,
        other => panic!("esperava Joined, recebeu {other:?}"),
    };
    recv_json(&mut sharer_ws).await; // drena o PeerJoined

    send_json(&mut sharer_ws, &ClientMessage::StartShare).await;
    assert_eq!(
        recv_json(&mut viewer_ws).await,
        ServerMessage::PeerStartedSharing {
            peer_id: sharer_id.clone(),
        }
    );

    // Answer — relayed to `to`, stamped with the sender's id as `from`.
    send_json(
        &mut viewer_ws,
        &ClientMessage::Answer {
            to: sharer_id.clone(),
            sdp: "answer-sdp".to_string(),
        },
    )
    .await;
    assert_eq!(
        recv_json(&mut sharer_ws).await,
        ServerMessage::Answer {
            from: viewer_id.clone(),
            sdp: "answer-sdp".to_string(),
        }
    );

    // IceCandidate — same relay, carries its extra fields through verbatim.
    send_json(
        &mut viewer_ws,
        &ClientMessage::IceCandidate {
            to: sharer_id.clone(),
            stream_owner: sharer_id.clone(),
            candidate: "candidate:1 1 udp".to_string(),
            sdp_mid: Some("0".to_string()),
            sdp_m_line_index: Some(0),
        },
    )
    .await;
    assert_eq!(
        recv_json(&mut sharer_ws).await,
        ServerMessage::IceCandidate {
            from: viewer_id.clone(),
            stream_owner: sharer_id.clone(),
            candidate: "candidate:1 1 udp".to_string(),
            sdp_mid: Some("0".to_string()),
            sdp_m_line_index: Some(0),
        }
    );

    // SetQuality — relayed to the sharer as `QualityRequested`.
    send_json(
        &mut viewer_ws,
        &ClientMessage::SetQuality {
            to: sharer_id.clone(),
            quality: QualityLevel::Low,
        },
    )
    .await;
    assert_eq!(
        recv_json(&mut sharer_ws).await,
        ServerMessage::QualityRequested {
            from: viewer_id.clone(),
            quality: QualityLevel::Low,
        }
    );

    // Ping — answered immediately, only to the sender.
    send_json(&mut viewer_ws, &ClientMessage::Ping).await;
    assert_eq!(recv_json(&mut viewer_ws).await, ServerMessage::Pong);

    // ReportLatency — broadcast to the whole room (sender included).
    send_json(&mut viewer_ws, &ClientMessage::ReportLatency { ms: 42 }).await;
    let expected_latency = ServerMessage::PeerLatency {
        peer_id: viewer_id.clone(),
        ms: 42,
    };
    assert_eq!(recv_json(&mut sharer_ws).await, expected_latency);
    assert_eq!(recv_json(&mut viewer_ws).await, expected_latency);

    // StopShare — broadcast to everyone but the sharer.
    send_json(&mut sharer_ws, &ClientMessage::StopShare).await;
    assert_eq!(
        recv_json(&mut viewer_ws).await,
        ServerMessage::PeerStoppedSharing { peer_id: sharer_id }
    );
}

/// The `JoinError::TooManyAttempts` arm: after `MAX_PASSWORD_ATTEMPTS`
/// wrong-password joins from one client, the next reply is
/// `TooManyAttempts`, not `AuthFailed`.
#[tokio::test]
async fn join_room_reports_too_many_attempts_after_the_lockout() {
    let url = spawn_test_server().await;

    let (mut creator_ws, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
    send_json(
        &mut creator_ws,
        &ClientMessage::CreateRoom {
            nick: "Ana".to_string(),
            password: Some("senha123".to_string()),
            room_name: "Sala".to_string(),
            color: "coral".to_string(),
            device_id: "device-ana".to_string(),
        },
    )
    .await;
    let room = match recv_json(&mut creator_ws).await {
        ServerMessage::Joined { room, .. } => room,
        other => panic!("esperava Joined, recebeu {other:?}"),
    };

    let (mut attacker_ws, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
    let wrong_join = ClientMessage::JoinRoom {
        room: room.clone(),
        nick: "Bia".to_string(),
        password: Some("errada".to_string()),
        color: "sky".to_string(),
        device_id: "device-bia".to_string(),
    };

    for _ in 0..MAX_PASSWORD_ATTEMPTS {
        send_json(&mut attacker_ws, &wrong_join).await;
        assert_eq!(recv_json(&mut attacker_ws).await, ServerMessage::AuthFailed);
    }

    send_json(&mut attacker_ws, &wrong_join).await;
    assert_eq!(
        recv_json(&mut attacker_ws).await,
        ServerMessage::TooManyAttempts
    );
}

/// The `JoinError::Full` arm: the `MAX_MEMBERS + 1`-th join is answered
/// with `RoomFull`.
#[tokio::test]
async fn join_room_reports_room_full_at_capacity() {
    let url = spawn_test_server().await;

    let (mut creator_ws, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
    send_json(
        &mut creator_ws,
        &ClientMessage::CreateRoom {
            nick: "Ana".to_string(),
            password: None,
            room_name: "Sala".to_string(),
            color: "coral".to_string(),
            device_id: "device-ana".to_string(),
        },
    )
    .await;
    let room = match recv_json(&mut creator_ws).await {
        ServerMessage::Joined { room, .. } => room,
        other => panic!("esperava Joined, recebeu {other:?}"),
    };

    // Fill the remaining slots. Each socket is left with its PeerJoined
    // broadcasts unread — the test only cares about the join replies.
    let mut members = Vec::new();
    for i in 1..MAX_MEMBERS {
        let (mut ws, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
        send_json(
            &mut ws,
            &ClientMessage::JoinRoom {
                room: room.clone(),
                nick: format!("member-{i}"),
                password: None,
                color: "sky".to_string(),
                device_id: format!("device-{i}"),
            },
        )
        .await;
        assert!(matches!(
            recv_json(&mut ws).await,
            ServerMessage::Joined { .. }
        ));
        members.push(ws);
    }

    let (mut overflow_ws, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
    send_json(
        &mut overflow_ws,
        &ClientMessage::JoinRoom {
            room,
            nick: "one-too-many".to_string(),
            password: None,
            color: "sky".to_string(),
            device_id: "device-overflow".to_string(),
        },
    )
    .await;
    assert_eq!(recv_json(&mut overflow_ws).await, ServerMessage::RoomFull);
}

#[tokio::test]
async fn second_create_room_on_the_same_socket_is_refused() {
    let (url, registry) = spawn_test_server_returning_registry(None).await;
    let (mut ws, _) = tokio_tungstenite::connect_async(&url).await.unwrap();

    send_json(
        &mut ws,
        &ClientMessage::CreateRoom {
            nick: "Ana".to_string(),
            password: None,
            room_name: "Sala da Ana".to_string(),
            color: "coral".to_string(),
            device_id: "device-ana".to_string(),
        },
    )
    .await;
    assert!(matches!(
        recv_json(&mut ws).await,
        ServerMessage::Joined { .. }
    ));

    send_json(
        &mut ws,
        &ClientMessage::CreateRoom {
            nick: "Ana".to_string(),
            password: None,
            room_name: "Outra sala".to_string(),
            color: "coral".to_string(),
            device_id: "device-ana".to_string(),
        },
    )
    .await;
    assert_eq!(recv_json(&mut ws).await, ServerMessage::AlreadyInRoom);
    assert_eq!(
        registry.room_count(),
        1,
        "a second CreateRoom on one socket must not create a second room"
    );
}

#[tokio::test]
async fn join_room_on_a_socket_that_already_created_one_is_refused() {
    let (url, _registry) = spawn_test_server_returning_registry(None).await;
    let (mut ws, _) = tokio_tungstenite::connect_async(&url).await.unwrap();

    send_json(
        &mut ws,
        &ClientMessage::CreateRoom {
            nick: "Ana".to_string(),
            password: None,
            room_name: "Sala da Ana".to_string(),
            color: "coral".to_string(),
            device_id: "device-ana".to_string(),
        },
    )
    .await;
    assert!(matches!(
        recv_json(&mut ws).await,
        ServerMessage::Joined { .. }
    ));

    send_json(
        &mut ws,
        &ClientMessage::JoinRoom {
            room: "SOMEROOM".to_string(),
            nick: "Ana".to_string(),
            password: None,
            color: "coral".to_string(),
            device_id: "device-ana".to_string(),
        },
    )
    .await;
    assert_eq!(recv_json(&mut ws).await, ServerMessage::AlreadyInRoom);
}

#[tokio::test]
async fn repeated_join_on_one_socket_does_not_orphan_members() {
    let (url, registry) = spawn_test_server_returning_registry(None).await;

    let (mut host, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
    send_json(
        &mut host,
        &ClientMessage::CreateRoom {
            nick: "Ana".to_string(),
            password: None,
            room_name: "Sala da Ana".to_string(),
            color: "coral".to_string(),
            device_id: "device-ana".to_string(),
        },
    )
    .await;
    let room = match recv_json(&mut host).await {
        ServerMessage::Joined { room, .. } => room,
        other => panic!("esperava Joined, recebeu {other:?}"),
    };

    let (mut joiner, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
    for device in ["device-b1", "device-b2", "device-b3"] {
        send_json(
            &mut joiner,
            &ClientMessage::JoinRoom {
                room: room.clone(),
                nick: "Bia".to_string(),
                password: None,
                color: "sky".to_string(),
                device_id: device.to_string(),
            },
        )
        .await;
        let _ = recv_json(&mut joiner).await;
    }

    let summary = registry
        .room_status(&room)
        .expect("room should still exist");
    assert_eq!(
        summary.member_count, 2,
        "extra JoinRoom messages on a bound socket must not add orphan members"
    );
}

#[tokio::test]
async fn a_frame_over_the_size_limit_closes_the_connection() {
    use futures_util::SinkExt;

    let url = spawn_test_server().await;
    let (mut ws, _) = tokio_tungstenite::connect_async(&url).await.unwrap();

    // Comfortably over MAX_MESSAGE_BYTES (256 KiB) in ws.rs — the server
    // must reject the frame and close rather than buffer it.
    let oversized = "x".repeat(300 * 1024);
    ws.send(Message::Text(oversized.into())).await.unwrap();

    // The next read either yields the server's close frame / an error, or
    // the stream ends — any of which means the connection was dropped.
    let closed = match tokio::time::timeout(RECV_TIMEOUT, ws.next()).await {
        Ok(None) | Ok(Some(Err(_))) => true,
        Ok(Some(Ok(Message::Close(_)))) => true,
        Ok(Some(Ok(other))) => panic!("expected a close, got {other:?}"),
        Err(_) => false,
    };
    assert!(closed, "an oversized frame must close the signaling socket");
}

#[tokio::test]
async fn handshake_is_rejected_from_an_origin_not_on_the_allowlist() {
    use screen_share_signaling::handshake::OriginPolicy;
    use tokio_tungstenite::tungstenite::http::Uri;
    use tokio_tungstenite::tungstenite::ClientRequestBuilder;

    let (url, _registry) = spawn_test_server_full(
        None,
        HandshakeConfig::new(OriginPolicy::parse(Some("https://app.example")), false),
    )
    .await;
    let uri: Uri = url.parse().unwrap();

    let evil = ClientRequestBuilder::new(uri.clone()).with_header("Origin", "https://evil.example");
    assert!(
        tokio_tungstenite::connect_async(evil).await.is_err(),
        "a cross-origin handshake must not upgrade"
    );

    let allowed = ClientRequestBuilder::new(uri).with_header("Origin", "https://app.example");
    assert!(
        tokio_tungstenite::connect_async(allowed).await.is_ok(),
        "the app's own origin must still upgrade"
    );
}
