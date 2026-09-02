//! Moved out of src/registry.rs (refactor Phase 4).

use std::time::Duration;

use screen_share_protocol::{LatencyInfo, MemberInfo, ServerMessage, WatcherInfo, MAX_MEMBERS};
use screen_share_signaling::auth::MAX_PASSWORD_LEN;
use screen_share_signaling::registry::*;

/// Bound on how long a registry broadcast may take to reach a member's
/// channel. Every path here is synchronous in-process work, so a correct
/// registry delivers immediately; bounding the wait turns a dropped
/// broadcast into a test failure instead of a hang — which also lets a
/// mutation run score a suppressed broadcast as caught rather than as an
/// inconclusive timeout.
const RECV_TIMEOUT: Duration = Duration::from_secs(5);

/// Await the next `ServerMessage` on `rx`, failing the test if none
/// arrives within [`RECV_TIMEOUT`] or the channel has closed.
async fn recv(rx: &mut MemberRx) -> ServerMessage {
    tokio::time::timeout(RECV_TIMEOUT, rx.recv())
        .await
        .expect("timed out waiting for a registry broadcast")
        .expect("registry channel closed while waiting for a broadcast")
}

#[tokio::test]
async fn create_room_registers_creator_and_returns_snapshot() {
    let registry = Registry::new();
    let (tx, _rx) = member_channel();

    let (room_code, snapshot) = registry
        .create_room(CreateRoomRequest {
            nick: "Ana".to_string(),
            color: "coral".to_string(),
            room_name: "Sala da Ana".to_string(),
            password: Some("senha123".to_string()),
            device_id: "device-tx".to_string(),
            client_key: "client-1".to_string(),
            sender: tx,
        })
        .expect("create_room should not hit a capacity limit in this test");

    assert_eq!(room_code.len(), ROOM_CODE_LENGTH);
    assert_eq!(
        snapshot.members,
        vec![MemberInfo {
            peer_id: snapshot.peer_id.clone(),
            nick: "Ana".to_string(),
            color: "coral".to_string()
        }]
    );
    assert_eq!(snapshot.room_name, "Sala da Ana");
    assert!(snapshot.active_sharers.is_empty());
}

#[tokio::test]
async fn join_room_not_found_returns_error() {
    let registry = Registry::new();
    let (tx, _rx) = member_channel();

    let result = registry.join_room(
        "NOPE0000",
        JoinRequest {
            nick: "Bia".to_string(),
            color: "sky".to_string(),
            password: Some("senha123".to_string()),
            device_id: "device-tx".to_string(),
            client_key: "client-1".to_string(),
            sender: tx,
        },
    );
    assert_eq!(result.unwrap_err(), JoinError::NotFound);
}

#[tokio::test]
async fn join_room_with_wrong_password_returns_error() {
    let registry = Registry::new();
    let (host_tx, _host_rx) = member_channel();
    let (viewer_tx, _viewer_rx) = member_channel();

    let (room_code, _snapshot) = registry
        .create_room(CreateRoomRequest {
            nick: "Ana".to_string(),
            color: "coral".to_string(),
            room_name: "Sala da Ana".to_string(),
            password: Some("senha123".to_string()),
            device_id: "device-host".to_string(),
            client_key: "client-1".to_string(),
            sender: host_tx,
        })
        .expect("create_room should not hit a capacity limit in this test");
    let result = registry.join_room(
        &room_code,
        JoinRequest {
            nick: "Bia".to_string(),
            color: "sky".to_string(),
            password: Some("senha-errada".to_string()),
            device_id: "device-viewer".to_string(),
            client_key: "client-1".to_string(),
            sender: viewer_tx,
        },
    );

    assert_eq!(result.unwrap_err(), JoinError::WrongPassword);
}

#[tokio::test]
async fn join_room_success_notifies_existing_members_and_includes_them_in_snapshot() {
    let registry = Registry::new();
    let (host_tx, mut host_rx) = member_channel();
    let (viewer_tx, _viewer_rx) = member_channel();

    let (room_code, creator_snapshot) = registry
        .create_room(CreateRoomRequest {
            nick: "Ana".to_string(),
            color: "coral".to_string(),
            room_name: "Sala da Ana".to_string(),
            password: Some("senha123".to_string()),
            device_id: "device-host".to_string(),
            client_key: "client-1".to_string(),
            sender: host_tx,
        })
        .expect("create_room should not hit a capacity limit in this test");
    let snapshot = registry
        .join_room(
            &room_code,
            JoinRequest {
                nick: "Bia".to_string(),
                color: "sky".to_string(),
                password: Some("senha123".to_string()),
                device_id: "device-viewer".to_string(),
                client_key: "client-1".to_string(),
                sender: viewer_tx,
            },
        )
        .unwrap();

    assert_eq!(snapshot.members.len(), 2);
    assert!(snapshot
        .members
        .iter()
        .any(|m| m.peer_id == creator_snapshot.peer_id && m.nick == "Ana"));
    assert!(snapshot
        .members
        .iter()
        .any(|m| m.peer_id == snapshot.peer_id && m.nick == "Bia"));

    let notification = recv(&mut host_rx).await;
    assert_eq!(
        notification,
        ServerMessage::PeerJoined {
            peer_id: snapshot.peer_id.clone(),
            nick: "Bia".to_string(),
            color: "sky".to_string()
        }
    );
}

#[tokio::test]
async fn join_room_full_returns_error() {
    let registry = Registry::new();
    let (host_tx, _host_rx) = member_channel();
    let (room_code, _snapshot) = registry
        .create_room(CreateRoomRequest {
            nick: "Membro0".to_string(),
            color: "coral".to_string(),
            room_name: "Sala da Membro0".to_string(),
            password: Some("senha123".to_string()),
            device_id: "device-host".to_string(),
            client_key: "client-1".to_string(),
            sender: host_tx,
        })
        .expect("create_room should not hit a capacity limit in this test");

    for i in 1..MAX_MEMBERS {
        let (tx, _rx) = member_channel();
        registry
            .join_room(
                &room_code,
                JoinRequest {
                    nick: format!("Membro{i}"),
                    color: "sky".to_string(),
                    password: Some("senha123".to_string()),
                    device_id: format!("device-membro-{i}"),
                    client_key: "client-1".to_string(),
                    sender: tx,
                },
            )
            .expect("deveria caber até MAX_MEMBERS");
    }

    let (extra_tx, _extra_rx) = member_channel();
    let result = registry.join_room(
        &room_code,
        JoinRequest {
            nick: "MembroExtra".to_string(),
            color: "sky".to_string(),
            password: Some("senha123".to_string()),
            device_id: "device-extra".to_string(),
            client_key: "client-1".to_string(),
            sender: extra_tx,
        },
    );
    assert_eq!(result.unwrap_err(), JoinError::Full);
}

#[tokio::test]
async fn join_room_from_same_device_kicks_the_previous_connection() {
    let registry = Registry::new();
    let (host_tx, mut host_rx) = member_channel();
    let (viewer_tx, mut viewer_rx) = member_channel();
    let (room_code, creator_snapshot) = registry
        .create_room(CreateRoomRequest {
            nick: "Ana".to_string(),
            color: "coral".to_string(),
            room_name: "Sala da Ana".to_string(),
            password: Some("senha123".to_string()),
            device_id: "device-ana".to_string(),
            client_key: "client-1".to_string(),
            sender: host_tx,
        })
        .expect("create_room should not hit a capacity limit in this test");
    registry
        .join_room(
            &room_code,
            JoinRequest {
                nick: "Bia".to_string(),
                color: "sky".to_string(),
                password: Some("senha123".to_string()),
                device_id: "device-viewer".to_string(),
                client_key: "client-1".to_string(),
                sender: viewer_tx,
            },
        )
        .unwrap();
    recv(&mut host_rx).await; // drain Bia's PeerJoined

    let (host_tx_2, mut host_rx_2) = member_channel();
    let snapshot = registry
        .join_room(
            &room_code,
            JoinRequest {
                nick: "AnaCelular".to_string(),
                color: "coral".to_string(),
                password: Some("senha123".to_string()),
                device_id: "device-ana".to_string(),
                client_key: "client-1".to_string(),
                sender: host_tx_2,
            },
        )
        .unwrap();

    assert_eq!(recv(&mut host_rx).await, ServerMessage::Kicked);

    assert_eq!(
        recv(&mut viewer_rx).await,
        ServerMessage::PeerLeft {
            peer_id: creator_snapshot.peer_id.clone()
        }
    );
    assert_eq!(
        recv(&mut viewer_rx).await,
        ServerMessage::PeerJoined {
            peer_id: snapshot.peer_id.clone(),
            nick: "AnaCelular".to_string(),
            color: "coral".to_string()
        }
    );

    assert_eq!(snapshot.members.len(), 2);
    assert!(snapshot
        .members
        .iter()
        .any(|m| m.peer_id == snapshot.peer_id && m.nick == "AnaCelular"));
    assert!(!snapshot
        .members
        .iter()
        .any(|m| m.peer_id == creator_snapshot.peer_id));

    assert!(host_rx_2.try_recv().is_err());
}

#[tokio::test]
async fn start_share_notifies_others_but_not_self() {
    let registry = Registry::new();
    let (host_tx, mut host_rx) = member_channel();
    let (viewer_tx, mut viewer_rx) = member_channel();

    let (room_code, creator_snapshot) = registry
        .create_room(CreateRoomRequest {
            nick: "Ana".to_string(),
            color: "coral".to_string(),
            room_name: "Sala da Ana".to_string(),
            password: Some("senha123".to_string()),
            device_id: "device-host".to_string(),
            client_key: "client-1".to_string(),
            sender: host_tx,
        })
        .expect("create_room should not hit a capacity limit in this test");
    registry
        .join_room(
            &room_code,
            JoinRequest {
                nick: "Bia".to_string(),
                color: "sky".to_string(),
                password: Some("senha123".to_string()),
                device_id: "device-viewer".to_string(),
                client_key: "client-1".to_string(),
                sender: viewer_tx,
            },
        )
        .unwrap();
    recv(&mut host_rx).await; // drain the PeerJoined

    registry.start_share(&room_code, &creator_snapshot.peer_id);

    let notification = recv(&mut viewer_rx).await;
    assert_eq!(
        notification,
        ServerMessage::PeerStartedSharing {
            peer_id: creator_snapshot.peer_id.clone()
        }
    );
    assert!(host_rx.try_recv().is_err());
}

#[tokio::test]
async fn stop_share_notifies_others() {
    let registry = Registry::new();
    let (host_tx, mut host_rx) = member_channel();
    let (viewer_tx, mut viewer_rx) = member_channel();

    let (room_code, creator_snapshot) = registry
        .create_room(CreateRoomRequest {
            nick: "Ana".to_string(),
            color: "coral".to_string(),
            room_name: "Sala da Ana".to_string(),
            password: Some("senha123".to_string()),
            device_id: "device-host".to_string(),
            client_key: "client-1".to_string(),
            sender: host_tx,
        })
        .expect("create_room should not hit a capacity limit in this test");
    registry
        .join_room(
            &room_code,
            JoinRequest {
                nick: "Bia".to_string(),
                color: "sky".to_string(),
                password: Some("senha123".to_string()),
                device_id: "device-viewer".to_string(),
                client_key: "client-1".to_string(),
                sender: viewer_tx,
            },
        )
        .unwrap();
    recv(&mut host_rx).await; // drain the PeerJoined

    registry.start_share(&room_code, &creator_snapshot.peer_id);
    recv(&mut viewer_rx).await; // drain the PeerStartedSharing

    registry.stop_share(&room_code, &creator_snapshot.peer_id);
    let notification = recv(&mut viewer_rx).await;
    assert_eq!(
        notification,
        ServerMessage::PeerStoppedSharing {
            peer_id: creator_snapshot.peer_id
        }
    );
}

#[tokio::test]
async fn leave_room_survives_when_members_remain() {
    let registry = Registry::new();
    let (host_tx, _host_rx) = member_channel();
    let (viewer_tx, mut viewer_rx) = member_channel();

    let (room_code, creator_snapshot) = registry
        .create_room(CreateRoomRequest {
            nick: "Ana".to_string(),
            color: "coral".to_string(),
            room_name: "Sala da Ana".to_string(),
            password: Some("senha123".to_string()),
            device_id: "device-host".to_string(),
            client_key: "client-1".to_string(),
            sender: host_tx,
        })
        .expect("create_room should not hit a capacity limit in this test");
    registry
        .join_room(
            &room_code,
            JoinRequest {
                nick: "Bia".to_string(),
                color: "sky".to_string(),
                password: Some("senha123".to_string()),
                device_id: "device-viewer".to_string(),
                client_key: "client-1".to_string(),
                sender: viewer_tx,
            },
        )
        .unwrap();

    registry.leave_room(&room_code, &creator_snapshot.peer_id);

    let notification = recv(&mut viewer_rx).await;
    assert_eq!(
        notification,
        ServerMessage::PeerLeft {
            peer_id: creator_snapshot.peer_id
        }
    );

    let (another_tx, _another_rx) = member_channel();
    assert!(registry
        .join_room(
            &room_code,
            JoinRequest {
                nick: "Caio".to_string(),
                color: "sky".to_string(),
                password: Some("senha123".to_string()),
                device_id: "device-another".to_string(),
                client_key: "client-1".to_string(),
                sender: another_tx
            }
        )
        .is_ok());
}

#[tokio::test]
async fn leave_room_keeps_an_emptied_room_joinable_during_the_grace_period() {
    let registry = Registry::new();
    let (host_tx, _host_rx) = member_channel();
    let (room_code, creator_snapshot) = registry
        .create_room(CreateRoomRequest {
            nick: "Ana".to_string(),
            color: "coral".to_string(),
            room_name: "Sala da Ana".to_string(),
            password: Some("senha123".to_string()),
            device_id: "device-host".to_string(),
            client_key: "client-1".to_string(),
            sender: host_tx,
        })
        .expect("create_room should not hit a capacity limit in this test");

    registry.leave_room(&room_code, &creator_snapshot.peer_id);

    let (tx, _rx) = member_channel();
    let result = registry.join_room(
        &room_code,
        JoinRequest {
            nick: "Bia".to_string(),
            color: "sky".to_string(),
            password: Some("senha123".to_string()),
            device_id: "device-tx".to_string(),
            client_key: "client-1".to_string(),
            sender: tx,
        },
    );
    assert!(
        result.is_ok(),
        "an emptied room should still be joinable right away"
    );
}

#[tokio::test(start_paused = true)]
async fn leave_room_removes_an_emptied_room_once_the_grace_period_elapses() {
    let registry = Registry::new();
    let (host_tx, _host_rx) = member_channel();
    let (room_code, creator_snapshot) = registry
        .create_room(CreateRoomRequest {
            nick: "Ana".to_string(),
            color: "coral".to_string(),
            room_name: "Sala da Ana".to_string(),
            password: Some("senha123".to_string()),
            device_id: "device-host".to_string(),
            client_key: "client-1".to_string(),
            sender: host_tx,
        })
        .expect("create_room should not hit a capacity limit in this test");

    registry.leave_room(&room_code, &creator_snapshot.peer_id);
    // `leave_room` only *schedules* the cleanup task — give the
    // single-threaded test runtime a turn to actually start it (so its
    // `sleep` registers against the current time) before advancing the
    // clock past the grace period, then again to let it finish running.
    tokio::task::yield_now().await;
    tokio::time::advance(EMPTY_ROOM_GRACE_PERIOD + Duration::from_secs(1)).await;
    tokio::task::yield_now().await;

    let (tx, _rx) = member_channel();
    let result = registry.join_room(
        &room_code,
        JoinRequest {
            nick: "Bia".to_string(),
            color: "sky".to_string(),
            password: Some("senha123".to_string()),
            device_id: "device-tx".to_string(),
            client_key: "client-1".to_string(),
            sender: tx,
        },
    );
    assert_eq!(result.unwrap_err(), JoinError::NotFound);
}

#[tokio::test(start_paused = true)]
async fn leave_room_does_not_remove_the_room_if_someone_rejoins_during_the_grace_period() {
    let registry = Registry::new();
    let (host_tx, _host_rx) = member_channel();
    let (room_code, creator_snapshot) = registry
        .create_room(CreateRoomRequest {
            nick: "Ana".to_string(),
            color: "coral".to_string(),
            room_name: "Sala da Ana".to_string(),
            password: Some("senha123".to_string()),
            device_id: "device-host".to_string(),
            client_key: "client-1".to_string(),
            sender: host_tx,
        })
        .expect("create_room should not hit a capacity limit in this test");

    registry.leave_room(&room_code, &creator_snapshot.peer_id);

    let (tx, _rx) = member_channel();
    registry
        .join_room(
            &room_code,
            JoinRequest {
                nick: "Bia".to_string(),
                color: "sky".to_string(),
                password: Some("senha123".to_string()),
                device_id: "device-tx".to_string(),
                client_key: "client-1".to_string(),
                sender: tx,
            },
        )
        .unwrap();

    tokio::time::advance(EMPTY_ROOM_GRACE_PERIOD + Duration::from_secs(1)).await;

    let status = registry.room_status(&room_code);
    assert!(
        status.is_some(),
        "the room should survive since it's no longer empty"
    );
}

#[tokio::test(start_paused = true)]
async fn a_rejoin_then_releave_reschedules_the_cleanup_instead_of_stacking_a_task() {
    // P3 follow-up: one cleanup task per room, not per emptying event.
    // The live task must notice a re-emptying and wait out a fresh grace
    // period rather than deleting the room early or letting a second task
    // pile up.
    let registry = Registry::new();
    let (host_tx, _host_rx) = member_channel();
    let (room_code, creator_snapshot) = registry
        .create_room(CreateRoomRequest {
            nick: "Ana".to_string(),
            color: "coral".to_string(),
            room_name: "Sala da Ana".to_string(),
            password: None,
            device_id: "device-host".to_string(),
            client_key: "client-1".to_string(),
            sender: host_tx,
        })
        .expect("create_room should not hit a capacity limit in this test");

    registry.leave_room(&room_code, &creator_snapshot.peer_id);
    tokio::task::yield_now().await;

    // Halfway through the grace period, someone rejoins and immediately
    // leaves again.
    tokio::time::advance(EMPTY_ROOM_GRACE_PERIOD / 2).await;
    let (tx, _rx) = member_channel();
    let rejoin = registry
        .join_room(
            &room_code,
            JoinRequest {
                nick: "Bia".to_string(),
                color: "sky".to_string(),
                password: None,
                device_id: "device-bia".to_string(),
                client_key: "client-1".to_string(),
                sender: tx,
            },
        )
        .unwrap();
    registry.leave_room(&room_code, &rejoin.peer_id);
    tokio::task::yield_now().await;

    // Just past the *first* leave's grace period — the room must still be
    // there, because it was re-emptied more recently.
    tokio::time::advance(EMPTY_ROOM_GRACE_PERIOD / 2 + Duration::from_secs(1)).await;
    tokio::task::yield_now().await;
    assert!(
        registry.room_status(&room_code).is_some(),
        "the room must not be deleted a full grace period before its last leave"
    );

    // Past the second leave's grace period — now it goes.
    tokio::time::advance(EMPTY_ROOM_GRACE_PERIOD).await;
    tokio::task::yield_now().await;
    assert!(
        registry.room_status(&room_code).is_none(),
        "once quiet for a full grace period the room is removed"
    );
}

#[tokio::test]
async fn leave_room_while_sharing_also_sends_peer_stopped_sharing() {
    let registry = Registry::new();
    let (host_tx, _host_rx) = member_channel();
    let (viewer_tx, mut viewer_rx) = member_channel();

    let (room_code, creator_snapshot) = registry
        .create_room(CreateRoomRequest {
            nick: "Ana".to_string(),
            color: "coral".to_string(),
            room_name: "Sala da Ana".to_string(),
            password: Some("senha123".to_string()),
            device_id: "device-host".to_string(),
            client_key: "client-1".to_string(),
            sender: host_tx,
        })
        .expect("create_room should not hit a capacity limit in this test");
    registry
        .join_room(
            &room_code,
            JoinRequest {
                nick: "Bia".to_string(),
                color: "sky".to_string(),
                password: Some("senha123".to_string()),
                device_id: "device-viewer".to_string(),
                client_key: "client-1".to_string(),
                sender: viewer_tx,
            },
        )
        .unwrap();
    registry.start_share(&room_code, &creator_snapshot.peer_id);
    recv(&mut viewer_rx).await; // drain the PeerStartedSharing

    registry.leave_room(&room_code, &creator_snapshot.peer_id);

    let left = recv(&mut viewer_rx).await;
    assert_eq!(
        left,
        ServerMessage::PeerLeft {
            peer_id: creator_snapshot.peer_id.clone()
        }
    );
    let stopped = recv(&mut viewer_rx).await;
    assert_eq!(
        stopped,
        ServerMessage::PeerStoppedSharing {
            peer_id: creator_snapshot.peer_id
        }
    );
}

#[tokio::test]
async fn add_watcher_notifies_sharer_and_broadcasts_count_to_everyone() {
    let registry = Registry::new();
    let (host_tx, mut host_rx) = member_channel();
    let (viewer_tx, mut viewer_rx) = member_channel();

    let (room_code, creator_snapshot) = registry
        .create_room(CreateRoomRequest {
            nick: "Ana".to_string(),
            color: "coral".to_string(),
            room_name: "Sala da Ana".to_string(),
            password: Some("senha123".to_string()),
            device_id: "device-host".to_string(),
            client_key: "client-1".to_string(),
            sender: host_tx,
        })
        .expect("create_room should not hit a capacity limit in this test");
    let viewer_snapshot = registry
        .join_room(
            &room_code,
            JoinRequest {
                nick: "Bia".to_string(),
                color: "sky".to_string(),
                password: Some("senha123".to_string()),
                device_id: "device-viewer".to_string(),
                client_key: "client-1".to_string(),
                sender: viewer_tx,
            },
        )
        .unwrap();
    recv(&mut host_rx).await; // drain the PeerJoined

    // `add_watcher` only takes effect for a peer that is actually sharing
    // (F07 hardening) — start the share first.
    registry.start_share(&room_code, &creator_snapshot.peer_id);
    recv(&mut viewer_rx).await; // drain the PeerStartedSharing

    registry.add_watcher(
        &room_code,
        &creator_snapshot.peer_id,
        &viewer_snapshot.peer_id,
    );

    assert_eq!(
        recv(&mut host_rx).await,
        ServerMessage::WatchRequested {
            from: viewer_snapshot.peer_id.clone()
        }
    );
    assert_eq!(
        recv(&mut host_rx).await,
        ServerMessage::WatchersChanged {
            sharer_id: creator_snapshot.peer_id.clone(),
            watchers: vec![viewer_snapshot.peer_id.clone()]
        }
    );
    assert_eq!(
        recv(&mut viewer_rx).await,
        ServerMessage::WatchersChanged {
            sharer_id: creator_snapshot.peer_id,
            watchers: vec![viewer_snapshot.peer_id]
        }
    );
}

#[tokio::test]
async fn remove_watcher_notifies_sharer_and_broadcasts_updated_count() {
    let registry = Registry::new();
    let (host_tx, mut host_rx) = member_channel();
    let (viewer_tx, mut viewer_rx) = member_channel();

    let (room_code, creator_snapshot) = registry
        .create_room(CreateRoomRequest {
            nick: "Ana".to_string(),
            color: "coral".to_string(),
            room_name: "Sala da Ana".to_string(),
            password: Some("senha123".to_string()),
            device_id: "device-host".to_string(),
            client_key: "client-1".to_string(),
            sender: host_tx,
        })
        .expect("create_room should not hit a capacity limit in this test");
    let viewer_snapshot = registry
        .join_room(
            &room_code,
            JoinRequest {
                nick: "Bia".to_string(),
                color: "sky".to_string(),
                password: Some("senha123".to_string()),
                device_id: "device-viewer".to_string(),
                client_key: "client-1".to_string(),
                sender: viewer_tx,
            },
        )
        .unwrap();
    recv(&mut host_rx).await; // drain the PeerJoined

    // `add_watcher` only takes effect for a peer that is actually sharing
    // (F07 hardening) — start the share first.
    registry.start_share(&room_code, &creator_snapshot.peer_id);
    recv(&mut viewer_rx).await; // drain the PeerStartedSharing

    registry.add_watcher(
        &room_code,
        &creator_snapshot.peer_id,
        &viewer_snapshot.peer_id,
    );
    recv(&mut host_rx).await; // drain the WatchRequested
    recv(&mut host_rx).await; // drain the WatchersChanged
    recv(&mut viewer_rx).await; // drain the WatchersChanged

    registry.remove_watcher(
        &room_code,
        &creator_snapshot.peer_id,
        &viewer_snapshot.peer_id,
    );

    assert_eq!(
        recv(&mut host_rx).await,
        ServerMessage::WatchStopped {
            from: viewer_snapshot.peer_id.clone()
        }
    );
    assert_eq!(
        recv(&mut host_rx).await,
        ServerMessage::WatchersChanged {
            sharer_id: creator_snapshot.peer_id.clone(),
            watchers: vec![]
        }
    );
    assert_eq!(
        recv(&mut viewer_rx).await,
        ServerMessage::WatchersChanged {
            sharer_id: creator_snapshot.peer_id,
            watchers: vec![]
        }
    );
}

#[tokio::test]
async fn join_room_snapshot_includes_watcher_info_for_active_sharers() {
    let registry = Registry::new();
    let (host_tx, _host_rx) = member_channel();
    let (viewer_tx, mut viewer_rx) = member_channel();

    let (room_code, creator_snapshot) = registry
        .create_room(CreateRoomRequest {
            nick: "Ana".to_string(),
            color: "coral".to_string(),
            room_name: "Sala da Ana".to_string(),
            password: Some("senha123".to_string()),
            device_id: "device-host".to_string(),
            client_key: "client-1".to_string(),
            sender: host_tx,
        })
        .expect("create_room should not hit a capacity limit in this test");
    let viewer_snapshot = registry
        .join_room(
            &room_code,
            JoinRequest {
                nick: "Bia".to_string(),
                color: "sky".to_string(),
                password: Some("senha123".to_string()),
                device_id: "device-viewer".to_string(),
                client_key: "client-1".to_string(),
                sender: viewer_tx,
            },
        )
        .unwrap();
    registry.start_share(&room_code, &creator_snapshot.peer_id);
    registry.add_watcher(
        &room_code,
        &creator_snapshot.peer_id,
        &viewer_snapshot.peer_id,
    );
    recv(&mut viewer_rx).await; // drain PeerStartedSharing
    recv(&mut viewer_rx).await; // drain WatchersChanged

    let (late_tx, _late_rx) = member_channel();
    let late_snapshot = registry
        .join_room(
            &room_code,
            JoinRequest {
                nick: "Caio".to_string(),
                color: "sky".to_string(),
                password: Some("senha123".to_string()),
                device_id: "device-late".to_string(),
                client_key: "client-1".to_string(),
                sender: late_tx,
            },
        )
        .unwrap();

    assert_eq!(
        late_snapshot.watcher_info,
        vec![WatcherInfo {
            sharer_id: creator_snapshot.peer_id,
            watchers: vec![viewer_snapshot.peer_id]
        }]
    );
}

#[tokio::test]
async fn report_latency_broadcasts_to_the_whole_room_including_the_reporter() {
    let registry = Registry::new();
    let (host_tx, mut host_rx) = member_channel();
    let (viewer_tx, mut viewer_rx) = member_channel();

    let (room_code, _) = registry
        .create_room(CreateRoomRequest {
            nick: "Ana".to_string(),
            color: "coral".to_string(),
            room_name: "Sala da Ana".to_string(),
            password: Some("senha123".to_string()),
            device_id: "device-host".to_string(),
            client_key: "client-1".to_string(),
            sender: host_tx,
        })
        .expect("create_room should not hit a capacity limit in this test");
    let viewer_snapshot = registry
        .join_room(
            &room_code,
            JoinRequest {
                nick: "Bia".to_string(),
                color: "sky".to_string(),
                password: Some("senha123".to_string()),
                device_id: "device-viewer".to_string(),
                client_key: "client-1".to_string(),
                sender: viewer_tx,
            },
        )
        .unwrap();
    recv(&mut host_rx).await; // drain PeerJoined

    registry.report_latency(&room_code, &viewer_snapshot.peer_id, 87);

    assert_eq!(
        recv(&mut host_rx).await,
        ServerMessage::PeerLatency {
            peer_id: viewer_snapshot.peer_id.clone(),
            ms: 87
        }
    );
    assert_eq!(
        recv(&mut viewer_rx).await,
        ServerMessage::PeerLatency {
            peer_id: viewer_snapshot.peer_id,
            ms: 87
        }
    );
}

#[tokio::test]
async fn join_room_snapshot_includes_latencies_reported_before_joining() {
    let registry = Registry::new();
    let (host_tx, _host_rx) = member_channel();
    let (viewer_tx, _viewer_rx) = member_channel();

    let (room_code, creator_snapshot) = registry
        .create_room(CreateRoomRequest {
            nick: "Ana".to_string(),
            color: "coral".to_string(),
            room_name: "Sala da Ana".to_string(),
            password: Some("senha123".to_string()),
            device_id: "device-host".to_string(),
            client_key: "client-1".to_string(),
            sender: host_tx,
        })
        .expect("create_room should not hit a capacity limit in this test");
    registry
        .join_room(
            &room_code,
            JoinRequest {
                nick: "Bia".to_string(),
                color: "sky".to_string(),
                password: Some("senha123".to_string()),
                device_id: "device-viewer".to_string(),
                client_key: "client-1".to_string(),
                sender: viewer_tx,
            },
        )
        .unwrap();
    registry.report_latency(&room_code, &creator_snapshot.peer_id, 12);

    let (late_tx, _late_rx) = member_channel();
    let late_snapshot = registry
        .join_room(
            &room_code,
            JoinRequest {
                nick: "Caio".to_string(),
                color: "sky".to_string(),
                password: Some("senha123".to_string()),
                device_id: "device-late".to_string(),
                client_key: "client-1".to_string(),
                sender: late_tx,
            },
        )
        .unwrap();

    assert_eq!(
        late_snapshot.latencies,
        vec![LatencyInfo {
            peer_id: creator_snapshot.peer_id,
            ms: 12
        }]
    );
}

#[tokio::test]
async fn leave_room_removes_the_leaver_from_watcher_lists_and_broadcasts_update() {
    let registry = Registry::new();
    let (host_tx, mut host_rx) = member_channel();
    let (viewer_tx, _viewer_rx) = member_channel();

    let (room_code, creator_snapshot) = registry
        .create_room(CreateRoomRequest {
            nick: "Ana".to_string(),
            color: "coral".to_string(),
            room_name: "Sala da Ana".to_string(),
            password: Some("senha123".to_string()),
            device_id: "device-host".to_string(),
            client_key: "client-1".to_string(),
            sender: host_tx,
        })
        .expect("create_room should not hit a capacity limit in this test");
    let viewer_snapshot = registry
        .join_room(
            &room_code,
            JoinRequest {
                nick: "Bia".to_string(),
                color: "sky".to_string(),
                password: Some("senha123".to_string()),
                device_id: "device-viewer".to_string(),
                client_key: "client-1".to_string(),
                sender: viewer_tx,
            },
        )
        .unwrap();
    recv(&mut host_rx).await; // drain PeerJoined

    // `add_watcher` only takes effect for a peer that is actually sharing
    // (F07 hardening) — start the share first. `_viewer_rx` swallows the
    // PeerStartedSharing broadcast.
    registry.start_share(&room_code, &creator_snapshot.peer_id);

    registry.add_watcher(
        &room_code,
        &creator_snapshot.peer_id,
        &viewer_snapshot.peer_id,
    );
    recv(&mut host_rx).await; // drain WatchRequested
    recv(&mut host_rx).await; // drain WatchersChanged

    registry.leave_room(&room_code, &viewer_snapshot.peer_id);

    assert_eq!(
        recv(&mut host_rx).await,
        ServerMessage::PeerLeft {
            peer_id: viewer_snapshot.peer_id.clone()
        }
    );
    assert_eq!(
        recv(&mut host_rx).await,
        ServerMessage::WatchersChanged {
            sharer_id: creator_snapshot.peer_id,
            watchers: vec![]
        }
    );
}

#[tokio::test]
async fn room_status_reports_name_and_member_count_for_existing_room() {
    let registry = Registry::new();
    let (host_tx, _host_rx) = member_channel();
    let (room_code, _snapshot) = registry
        .create_room(CreateRoomRequest {
            nick: "Ana".to_string(),
            color: "coral".to_string(),
            room_name: "Sala da Ana".to_string(),
            password: Some("senha123".to_string()),
            device_id: "device-host".to_string(),
            client_key: "client-1".to_string(),
            sender: host_tx,
        })
        .expect("create_room should not hit a capacity limit in this test");

    let (viewer_tx, _viewer_rx) = member_channel();
    registry
        .join_room(
            &room_code,
            JoinRequest {
                nick: "Bia".to_string(),
                color: "sky".to_string(),
                password: Some("senha123".to_string()),
                device_id: "device-viewer".to_string(),
                client_key: "client-1".to_string(),
                sender: viewer_tx,
            },
        )
        .unwrap();

    let status = registry.room_status(&room_code).unwrap();
    assert_eq!(status.name, "Sala da Ana");
    assert_eq!(status.member_count, 2);
    assert!(status.requires_password);
}

#[tokio::test]
async fn room_status_is_none_for_unknown_room() {
    let registry = Registry::new();
    assert!(registry.room_status("NOPE0000").is_none());
}

#[tokio::test]
async fn create_room_without_password_lets_anyone_join() {
    let registry = Registry::new();
    let (host_tx, _host_rx) = member_channel();
    let (room_code, _snapshot) = registry
        .create_room(CreateRoomRequest {
            nick: "Ana".to_string(),
            color: "coral".to_string(),
            room_name: "Sala da Ana".to_string(),
            password: None,
            device_id: "device-host".to_string(),
            client_key: "client-1".to_string(),
            sender: host_tx,
        })
        .expect("create_room should not hit a capacity limit in this test");

    assert!(!registry.room_status(&room_code).unwrap().requires_password);

    let (viewer_tx, _viewer_rx) = member_channel();
    let result = registry.join_room(
        &room_code,
        JoinRequest {
            nick: "Bia".to_string(),
            color: "sky".to_string(),
            password: None,
            device_id: "device-viewer".to_string(),
            client_key: "client-1".to_string(),
            sender: viewer_tx,
        },
    );
    assert!(result.is_ok());
}

#[tokio::test]
async fn create_room_with_an_empty_password_behaves_as_no_password() {
    let registry = Registry::new();
    let (host_tx, _host_rx) = member_channel();
    let (room_code, _snapshot) = registry
        .create_room(CreateRoomRequest {
            nick: "Ana".to_string(),
            color: "coral".to_string(),
            room_name: "Sala da Ana".to_string(),
            password: Some(String::new()),
            device_id: "device-host".to_string(),
            client_key: "client-1".to_string(),
            sender: host_tx,
        })
        .expect("create_room should not hit a capacity limit in this test");

    assert!(!registry.room_status(&room_code).unwrap().requires_password);
}

#[tokio::test]
async fn join_room_with_a_password_fails_without_one_when_the_room_requires_it() {
    let registry = Registry::new();
    let (host_tx, _host_rx) = member_channel();
    let (room_code, _snapshot) = registry
        .create_room(CreateRoomRequest {
            nick: "Ana".to_string(),
            color: "coral".to_string(),
            room_name: "Sala da Ana".to_string(),
            password: Some("senha123".to_string()),
            device_id: "device-host".to_string(),
            client_key: "client-1".to_string(),
            sender: host_tx,
        })
        .expect("create_room should not hit a capacity limit in this test");

    let (viewer_tx, _viewer_rx) = member_channel();
    let result = registry.join_room(
        &room_code,
        JoinRequest {
            nick: "Bia".to_string(),
            color: "sky".to_string(),
            password: None,
            device_id: "device-viewer".to_string(),
            client_key: "client-1".to_string(),
            sender: viewer_tx,
        },
    );
    assert_eq!(result.unwrap_err(), JoinError::WrongPassword);
}

#[tokio::test]
async fn join_room_locks_out_after_too_many_wrong_passwords() {
    let registry = Registry::new();
    let (host_tx, _host_rx) = member_channel();
    let (room_code, _snapshot) = registry
        .create_room(CreateRoomRequest {
            nick: "Ana".to_string(),
            color: "coral".to_string(),
            room_name: "Sala da Ana".to_string(),
            password: Some("senha123".to_string()),
            device_id: "device-host".to_string(),
            client_key: "client-1".to_string(),
            sender: host_tx,
        })
        .expect("create_room should not hit a capacity limit in this test");

    for _ in 0..MAX_PASSWORD_ATTEMPTS {
        let (tx, _rx) = member_channel();
        let result = registry.join_room(
            &room_code,
            JoinRequest {
                nick: "Bia".to_string(),
                color: "sky".to_string(),
                password: Some("senha-errada".to_string()),
                device_id: "device-viewer".to_string(),
                client_key: "client-1".to_string(),
                sender: tx,
            },
        );
        assert_eq!(result.unwrap_err(), JoinError::WrongPassword);
    }

    // Even the correct password is rejected once the room is locked out —
    // otherwise a lucky guess at the tail end of a brute-force would
    // still get in.
    let (tx, _rx) = member_channel();
    let result = registry.join_room(
        &room_code,
        JoinRequest {
            nick: "Bia".to_string(),
            color: "sky".to_string(),
            password: Some("senha123".to_string()),
            device_id: "device-viewer".to_string(),
            client_key: "client-1".to_string(),
            sender: tx,
        },
    );
    assert_eq!(result.unwrap_err(), JoinError::TooManyAttempts);
}

#[tokio::test(start_paused = true)]
async fn join_room_lockout_clears_after_the_attempt_window_elapses() {
    let registry = Registry::new();
    let (host_tx, _host_rx) = member_channel();
    let (room_code, _snapshot) = registry
        .create_room(CreateRoomRequest {
            nick: "Ana".to_string(),
            color: "coral".to_string(),
            room_name: "Sala da Ana".to_string(),
            password: Some("senha123".to_string()),
            device_id: "device-host".to_string(),
            client_key: "client-1".to_string(),
            sender: host_tx,
        })
        .expect("create_room should not hit a capacity limit in this test");

    for _ in 0..MAX_PASSWORD_ATTEMPTS {
        let (tx, _rx) = member_channel();
        registry
            .join_room(
                &room_code,
                JoinRequest {
                    nick: "Bia".to_string(),
                    color: "sky".to_string(),
                    password: Some("senha-errada".to_string()),
                    device_id: "device-viewer".to_string(),
                    client_key: "client-1".to_string(),
                    sender: tx,
                },
            )
            .unwrap_err();
    }

    tokio::time::advance(PASSWORD_ATTEMPT_WINDOW + Duration::from_secs(1)).await;

    let (tx, _rx) = member_channel();
    let result = registry.join_room(
        &room_code,
        JoinRequest {
            nick: "Bia".to_string(),
            color: "sky".to_string(),
            password: Some("senha123".to_string()),
            device_id: "device-viewer".to_string(),
            client_key: "client-1".to_string(),
            sender: tx,
        },
    );
    assert!(
        result.is_ok(),
        "the lockout should clear once the attempt window has elapsed"
    );
}

#[tokio::test(start_paused = true)]
async fn join_room_lockout_clears_exactly_at_the_attempt_window_boundary() {
    // Pins the sliding window's boundary: an attempt whose age is exactly
    // `PASSWORD_ATTEMPT_WINDOW` is "older than the window" and must be
    // dropped, so the lockout clears at the boundary, not one instant
    // after it.
    let registry = Registry::new();
    let (host_tx, _host_rx) = member_channel();
    let (room_code, _snapshot) = registry
        .create_room(CreateRoomRequest {
            nick: "Ana".to_string(),
            color: "coral".to_string(),
            room_name: "Sala da Ana".to_string(),
            password: Some("senha123".to_string()),
            device_id: "device-host".to_string(),
            client_key: "client-1".to_string(),
            sender: host_tx,
        })
        .expect("create_room should not hit a capacity limit in this test");

    for _ in 0..MAX_PASSWORD_ATTEMPTS {
        let (tx, _rx) = member_channel();
        registry
            .join_room(
                &room_code,
                JoinRequest {
                    nick: "Bia".to_string(),
                    color: "sky".to_string(),
                    password: Some("senha-errada".to_string()),
                    device_id: "device-viewer".to_string(),
                    client_key: "client-1".to_string(),
                    sender: tx,
                },
            )
            .unwrap_err();
    }

    tokio::time::advance(PASSWORD_ATTEMPT_WINDOW).await;

    let (tx, _rx) = member_channel();
    let result = registry.join_room(
        &room_code,
        JoinRequest {
            nick: "Bia".to_string(),
            color: "sky".to_string(),
            password: Some("senha123".to_string()),
            device_id: "device-viewer".to_string(),
            client_key: "client-1".to_string(),
            sender: tx,
        },
    );
    assert!(
        result.is_ok(),
        "an attempt exactly one window old must not count toward the lockout"
    );
}

#[tokio::test]
async fn join_room_wrong_password_attempts_do_not_lock_out_other_rooms() {
    let registry = Registry::new();
    let (host_a_tx, _host_a_rx) = member_channel();
    let (room_a, _) = registry
        .create_room(CreateRoomRequest {
            nick: "Ana".to_string(),
            color: "coral".to_string(),
            room_name: "Sala da Ana".to_string(),
            password: Some("senha123".to_string()),
            device_id: "device-a".to_string(),
            client_key: "client-1".to_string(),
            sender: host_a_tx,
        })
        .expect("create_room should not hit a capacity limit in this test");
    let (host_b_tx, _host_b_rx) = member_channel();
    let (room_b, _) = registry
        .create_room(CreateRoomRequest {
            nick: "Caio".to_string(),
            color: "sky".to_string(),
            room_name: "Sala do Caio".to_string(),
            password: Some("outra-senha".to_string()),
            device_id: "device-b".to_string(),
            client_key: "client-1".to_string(),
            sender: host_b_tx,
        })
        .expect("create_room should not hit a capacity limit in this test");

    for _ in 0..MAX_PASSWORD_ATTEMPTS {
        let (tx, _rx) = member_channel();
        registry
            .join_room(
                &room_a,
                JoinRequest {
                    nick: "Bia".to_string(),
                    color: "sky".to_string(),
                    password: Some("senha-errada".to_string()),
                    device_id: "device-viewer".to_string(),
                    client_key: "client-1".to_string(),
                    sender: tx,
                },
            )
            .unwrap_err();
    }

    let (tx, _rx) = member_channel();
    let result = registry.join_room(
        &room_b,
        JoinRequest {
            nick: "Bia".to_string(),
            color: "sky".to_string(),
            password: Some("outra-senha".to_string()),
            device_id: "device-viewer".to_string(),
            client_key: "client-1".to_string(),
            sender: tx,
        },
    );
    assert!(
        result.is_ok(),
        "a lockout on one room must not affect another"
    );
}

#[tokio::test]
async fn join_room_wrong_password_attempts_from_one_client_do_not_lock_out_another_client_in_the_same_room(
) {
    let registry = Registry::new();
    let (host_tx, _host_rx) = member_channel();
    let (room_code, _snapshot) = registry
        .create_room(CreateRoomRequest {
            nick: "Ana".to_string(),
            color: "coral".to_string(),
            room_name: "Sala da Ana".to_string(),
            password: Some("senha123".to_string()),
            device_id: "device-host".to_string(),
            client_key: "client-1".to_string(),
            sender: host_tx,
        })
        .expect("create_room should not hit a capacity limit in this test");

    // An attacker at "client-attacker" burns through the attempt budget...
    for _ in 0..MAX_PASSWORD_ATTEMPTS {
        let (tx, _rx) = member_channel();
        registry
            .join_room(
                &room_code,
                JoinRequest {
                    nick: "Atacante".to_string(),
                    color: "sky".to_string(),
                    password: Some("senha-errada".to_string()),
                    device_id: "device-attacker".to_string(),
                    client_key: "client-attacker".to_string(),
                    sender: tx,
                },
            )
            .unwrap_err();
    }

    // ...but a different client joining the same room with the right
    // password is unaffected — the lockout must not be room-wide.
    let (tx, _rx) = member_channel();
    let result = registry.join_room(
        &room_code,
        JoinRequest {
            nick: "Bia".to_string(),
            color: "sky".to_string(),
            password: Some("senha123".to_string()),
            device_id: "device-viewer".to_string(),
            client_key: "client-legit".to_string(),
            sender: tx,
        },
    );
    assert!(
        result.is_ok(),
        "a lockout on one client must not affect another client joining the same room"
    );
}

/// Minimal `CreateRoomRequest` with a throwaway channel and no password
/// (skips argon2 hashing so capacity loops stay fast), varying only the
/// `client_key`.
fn create_request(client_key: &str) -> CreateRoomRequest {
    let (tx, _rx) = member_channel();
    CreateRoomRequest {
        nick: "Ana".to_string(),
        color: "coral".to_string(),
        room_name: "Sala".to_string(),
        password: None,
        device_id: "device".to_string(),
        client_key: client_key.to_string(),
        sender: tx,
    }
}

#[tokio::test]
async fn create_room_rejects_a_client_past_its_per_client_room_cap() {
    let registry = Registry::new();

    for _ in 0..MAX_ROOMS_PER_CLIENT {
        registry
            .create_room(create_request("noisy-client"))
            .expect("under the per-client cap");
    }

    assert!(
        matches!(
            registry.create_room(create_request("noisy-client")),
            Err(CreateRoomError::AtCapacity)
        ),
        "the room past a client's per-client cap must be refused"
    );
    assert_eq!(registry.room_count(), MAX_ROOMS_PER_CLIENT);

    // A different client is unaffected by another client's cap.
    assert!(
        registry.create_room(create_request("other-client")).is_ok(),
        "the per-client cap must not be global"
    );
}

#[tokio::test]
async fn create_room_does_not_count_an_emptied_room_against_the_per_client_cap() {
    let registry = Registry::new();

    // Fill the client's per-client budget, keeping the first room's handle.
    let (first_code, first_snapshot) = registry
        .create_room(create_request("busy-client"))
        .expect("first create is under the cap");
    for _ in 1..MAX_ROOMS_PER_CLIENT {
        registry
            .create_room(create_request("busy-client"))
            .expect("still under the per-client cap");
    }
    assert!(
        matches!(
            registry.create_room(create_request("busy-client")),
            Err(CreateRoomError::AtCapacity)
        ),
        "the client is at its per-client cap"
    );

    // Leaving empties that room (its creator was the only member). It now
    // lingers only for the grace period and must free the client's slot,
    // rather than blocking a fresh create until cleanup runs.
    registry.leave_room(&first_code, &first_snapshot.peer_id);

    assert!(
        registry.create_room(create_request("busy-client")).is_ok(),
        "an emptied room must not keep counting against the per-client cap"
    );
}

#[tokio::test]
async fn create_room_rejects_everyone_once_the_global_room_cap_is_reached() {
    let registry = Registry::new();

    for i in 0..MAX_ROOMS {
        registry
            .create_room(create_request(&format!("client-{i}")))
            .expect("under the global cap");
    }

    assert_eq!(registry.room_count(), MAX_ROOMS);
    assert!(
        matches!(
            registry.create_room(create_request("fresh-client")),
            Err(CreateRoomError::AtCapacity)
        ),
        "no client may create a room once the registry is at MAX_ROOMS"
    );
}

#[test]
fn try_acquire_connection_stops_handing_out_slots_at_the_cap() {
    let registry = Registry::new();

    let guards: Vec<_> = (0..MAX_WS_CONNECTIONS)
        .map(|_| {
            registry
                .try_acquire_connection()
                .expect("slots available below the cap")
        })
        .collect();

    assert!(
        registry.try_acquire_connection().is_none(),
        "no slot past MAX_WS_CONNECTIONS"
    );

    drop(guards.into_iter().next());
    assert!(
        registry.try_acquire_connection().is_some(),
        "dropping a guard frees exactly one slot"
    );
}

#[tokio::test]
async fn create_room_rejects_a_bad_nick_room_name_or_colour() {
    let registry = Registry::new();
    let base = || {
        let (tx, _rx) = member_channel();
        CreateRoomRequest {
            nick: "Ana".to_string(),
            color: "coral".to_string(),
            room_name: "Sala".to_string(),
            password: None,
            device_id: "device".to_string(),
            client_key: "client-1".to_string(),
            sender: tx,
        }
    };

    let long_nick = CreateRoomRequest {
        nick: "a".repeat(200),
        ..base()
    };
    assert_eq!(
        registry.create_room(long_nick).unwrap_err(),
        CreateRoomError::InvalidInput
    );

    let bidi_name = CreateRoomRequest {
        room_name: "Diretoria \u{202E}X".to_string(),
        ..base()
    };
    assert_eq!(
        registry.create_room(bidi_name).unwrap_err(),
        CreateRoomError::InvalidInput
    );

    let odd_colour = CreateRoomRequest {
        color: "chartreuse".to_string(),
        ..base()
    };
    assert_eq!(
        registry.create_room(odd_colour).unwrap_err(),
        CreateRoomError::InvalidInput
    );

    assert_eq!(registry.room_count(), 0, "no bad request created a room");
}

#[tokio::test]
async fn create_room_rejects_a_password_longer_than_the_limit() {
    let registry = Registry::new();
    let (tx, _rx) = member_channel();
    let result = registry.create_room(CreateRoomRequest {
        nick: "Ana".to_string(),
        color: "coral".to_string(),
        room_name: "Sala".to_string(),
        password: Some("a".repeat(MAX_PASSWORD_LEN + 1)),
        device_id: "device".to_string(),
        client_key: "client-1".to_string(),
        sender: tx,
    });
    assert_eq!(result.unwrap_err(), CreateRoomError::InvalidInput);
    assert_eq!(
        registry.room_count(),
        0,
        "the over-long password created no room"
    );

    let (tx, _rx) = member_channel();
    let ok = registry.create_room(CreateRoomRequest {
        nick: "Ana".to_string(),
        color: "coral".to_string(),
        room_name: "Sala".to_string(),
        password: Some("a".repeat(MAX_PASSWORD_LEN)),
        device_id: "device".to_string(),
        client_key: "client-1".to_string(),
        sender: tx,
    });
    assert!(ok.is_ok(), "a password exactly at the limit is accepted");
}

#[tokio::test]
async fn join_room_rejects_a_password_longer_than_the_limit() {
    let registry = Registry::new();
    let (host_tx, _host_rx) = member_channel();
    let (room_code, _snapshot) = registry
        .create_room(CreateRoomRequest {
            nick: "Ana".to_string(),
            color: "coral".to_string(),
            room_name: "Sala da Ana".to_string(),
            password: Some("senha123".to_string()),
            device_id: "device-host".to_string(),
            client_key: "client-1".to_string(),
            sender: host_tx,
        })
        .expect("create_room should not hit a capacity limit in this test");

    let (tx, _rx) = member_channel();
    let result = registry.join_room(
        &room_code,
        JoinRequest {
            nick: "Bia".to_string(),
            color: "sky".to_string(),
            password: Some("a".repeat(MAX_PASSWORD_LEN + 1)),
            device_id: "device-viewer".to_string(),
            client_key: "client-1".to_string(),
            sender: tx,
        },
    );
    assert_eq!(result.unwrap_err(), JoinError::InvalidInput);
}

#[tokio::test]
async fn join_room_with_an_empty_device_id_does_not_kick_another_empty_device_id() {
    let registry = Registry::new();
    let (host_tx, mut host_rx) = member_channel();
    let (room_code, _snapshot) = registry
        .create_room(CreateRoomRequest {
            nick: "Ana".to_string(),
            color: "coral".to_string(),
            room_name: "Sala".to_string(),
            password: None,
            device_id: String::new(),
            client_key: "client-1".to_string(),
            sender: host_tx,
        })
        .expect("create should succeed");

    let (bia_tx, _bia_rx) = member_channel();
    let bia = registry
        .join_room(
            &room_code,
            JoinRequest {
                nick: "Bia".to_string(),
                color: "sky".to_string(),
                password: None,
                device_id: String::new(),
                client_key: "client-1".to_string(),
                sender: bia_tx,
            },
        )
        .expect("Bia joins");

    // Ana must see Bia join, not receive a Kicked for herself.
    assert_eq!(
        recv(&mut host_rx).await,
        ServerMessage::PeerJoined {
            peer_id: bia.peer_id.clone(),
            nick: "Bia".to_string(),
            color: "sky".to_string(),
        }
    );
    assert!(
        host_rx.try_recv().is_err(),
        "no Kicked for an empty device_id collision"
    );
    assert_eq!(registry.room_status(&room_code).unwrap().member_count, 2);
}

#[tokio::test]
async fn join_room_with_a_real_device_id_still_kicks_the_previous_connection() {
    let registry = Registry::new();
    let (host_tx, _host_rx) = member_channel();
    let (room_code, _snapshot) = registry
        .create_room(CreateRoomRequest {
            nick: "Ana".to_string(),
            color: "coral".to_string(),
            room_name: "Sala".to_string(),
            password: None,
            device_id: "device-shared".to_string(),
            client_key: "client-1".to_string(),
            sender: host_tx,
        })
        .expect("create should succeed");

    let (first_tx, mut first_rx) = member_channel();
    registry
        .join_room(
            &room_code,
            JoinRequest {
                nick: "Bia".to_string(),
                color: "sky".to_string(),
                password: None,
                device_id: "device-bia".to_string(),
                client_key: "client-1".to_string(),
                sender: first_tx,
            },
        )
        .expect("Bia joins");

    let (second_tx, _second_rx) = member_channel();
    registry
        .join_room(
            &room_code,
            JoinRequest {
                nick: "BiaCelular".to_string(),
                color: "sky".to_string(),
                password: None,
                device_id: "device-bia".to_string(),
                client_key: "client-1".to_string(),
                sender: second_tx,
            },
        )
        .expect("Bia rejoins from the same device");

    assert_eq!(recv(&mut first_rx).await, ServerMessage::Kicked);
}

#[tokio::test]
async fn add_watcher_ignores_a_sharer_id_that_is_not_a_member() {
    let registry = Registry::new();
    let (host_tx, mut host_rx) = member_channel();
    let (room_code, _snapshot) = registry
        .create_room(CreateRoomRequest {
            nick: "Ana".to_string(),
            color: "coral".to_string(),
            room_name: "Sala".to_string(),
            password: None,
            device_id: "device-host".to_string(),
            client_key: "client-1".to_string(),
            sender: host_tx,
        })
        .expect("create should succeed");

    registry.add_watcher(&room_code, "ghost-peer", "some-viewer");

    // No WatchersChanged (or anything else) should have been broadcast.
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(100), host_rx.recv())
            .await
            .is_err(),
        "watching a non-member must be a no-op"
    );
}

/// F07 hardening: `add_watcher` gates on the target actually sharing, not
/// just on room membership. Without that check a member could `WatchShare`
/// an idle co-member, which fired a `WatchRequested` at them (opening an
/// `RTCPeerConnection` and leaking host/srflx ICE) and, via
/// `watch_related`, opened `relay_peer_signal` between the two for
/// unsolicited offers.
#[tokio::test]
async fn add_watcher_ignores_a_member_that_is_not_sharing() {
    let registry = Registry::new();
    let (host_tx, mut host_rx) = member_channel();
    let (viewer_tx, mut viewer_rx) = member_channel();

    let (room_code, host_snapshot) = registry
        .create_room(CreateRoomRequest {
            nick: "Ana".to_string(),
            color: "coral".to_string(),
            room_name: "Sala".to_string(),
            password: None,
            device_id: "device-host".to_string(),
            client_key: "client-1".to_string(),
            sender: host_tx,
        })
        .expect("create should succeed");
    let viewer_snapshot = registry
        .join_room(
            &room_code,
            JoinRequest {
                nick: "Bia".to_string(),
                color: "sky".to_string(),
                password: None,
                device_id: "device-viewer".to_string(),
                client_key: "client-1".to_string(),
                sender: viewer_tx,
            },
        )
        .unwrap();
    recv(&mut host_rx).await; // drain PeerJoined

    // The host never called `start_share`.
    registry.add_watcher(&room_code, &host_snapshot.peer_id, &viewer_snapshot.peer_id);

    // No WatchRequested to the "sharer", no WatchersChanged to anyone.
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(100), host_rx.recv())
            .await
            .is_err(),
        "watching a member that isn't sharing must be a no-op"
    );
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(100), viewer_rx.recv())
            .await
            .is_err(),
        "no WatchersChanged is broadcast for a non-sharer"
    );

    // The relay gate stays shut: an Offer between the two is still dropped.
    registry.relay_peer_signal(
        &room_code,
        &viewer_snapshot.peer_id,
        &host_snapshot.peer_id,
        ServerMessage::Offer {
            from: viewer_snapshot.peer_id.clone(),
            sdp: "v=0".to_string(),
        },
    );
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(100), host_rx.recv())
            .await
            .is_err(),
        "relay_peer_signal must not forward without a real watch relationship"
    );
}

#[tokio::test]
async fn report_latency_drops_an_implausible_value() {
    let registry = Registry::new();
    let (host_tx, mut host_rx) = member_channel();
    let (room_code, snapshot) = registry
        .create_room(CreateRoomRequest {
            nick: "Ana".to_string(),
            color: "coral".to_string(),
            room_name: "Sala".to_string(),
            password: None,
            device_id: "device-host".to_string(),
            client_key: "client-1".to_string(),
            sender: host_tx,
        })
        .expect("create should succeed");

    registry.report_latency(&room_code, &snapshot.peer_id, MAX_PLAUSIBLE_LATENCY_MS + 1);
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(100), host_rx.recv())
            .await
            .is_err(),
        "an implausible latency must not be rebroadcast"
    );

    // The threshold itself is still plausible — dropped only when strictly
    // above it (guards `>` vs `>=`).
    registry.report_latency(&room_code, &snapshot.peer_id, MAX_PLAUSIBLE_LATENCY_MS);
    assert!(matches!(
        recv(&mut host_rx).await,
        ServerMessage::PeerLatency { ms, .. } if ms == MAX_PLAUSIBLE_LATENCY_MS
    ));

    registry.report_latency(&room_code, &snapshot.peer_id, 42);
    assert!(matches!(
        recv(&mut host_rx).await,
        ServerMessage::PeerLatency { ms: 42, .. }
    ));
}
