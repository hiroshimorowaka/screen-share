use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use rand::RngExt;
use tokio::sync::mpsc::UnboundedSender;
use uuid::Uuid;

use super::protocol::ServerMessage;

struct Peer {
    sender: UnboundedSender<ServerMessage>,
}

struct Room {
    host: String,
    peers: HashMap<String, Peer>,
}

#[derive(Clone, Default)]
pub struct Registry {
    rooms: Arc<Mutex<HashMap<String, Room>>>,
}

impl Registry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn create_room(&self, host_sender: UnboundedSender<ServerMessage>) -> (String, String) {
        let room_code = generate_room_code();
        let host_id = Uuid::new_v4().to_string();

        let mut peers = HashMap::new();
        peers.insert(host_id.clone(), Peer { sender: host_sender });

        let mut rooms = self.rooms.lock().unwrap();
        rooms.insert(room_code.clone(), Room { host: host_id.clone(), peers });

        (room_code, host_id)
    }

    pub fn join_room(
        &self,
        room_code: &str,
        sender: UnboundedSender<ServerMessage>,
    ) -> Option<(String, String)> {
        let mut rooms = self.rooms.lock().unwrap();
        let room = rooms.get_mut(room_code)?;

        let peer_id = Uuid::new_v4().to_string();
        room.peers.insert(peer_id.clone(), Peer { sender });

        let host_id = room.host.clone();
        if let Some(host_peer) = room.peers.get(&host_id) {
            let _ = host_peer.sender.send(ServerMessage::PeerJoined { peer_id: peer_id.clone() });
        }

        Some((peer_id, host_id))
    }

    pub fn relay(&self, room_code: &str, to: &str, message: ServerMessage) {
        let rooms = self.rooms.lock().unwrap();
        if let Some(room) = rooms.get(room_code) {
            if let Some(peer) = room.peers.get(to) {
                let _ = peer.sender.send(message);
            }
        }
    }

    pub fn leave_room(&self, room_code: &str, peer_id: &str) {
        let mut rooms = self.rooms.lock().unwrap();
        let mut close_room = false;

        if let Some(room) = rooms.get_mut(room_code) {
            let is_host = peer_id == room.host;
            room.peers.remove(peer_id);

            if is_host {
                for peer in room.peers.values() {
                    let _ = peer.sender.send(ServerMessage::RoomClosed);
                }
                close_room = true;
            } else {
                for peer in room.peers.values() {
                    let _ = peer.sender.send(ServerMessage::PeerLeft { peer_id: peer_id.to_string() });
                }
                if room.peers.is_empty() {
                    close_room = true;
                }
            }
        }

        if close_room {
            rooms.remove(room_code);
        }
    }
}

fn generate_room_code() -> String {
    const CHARS: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    let mut rng = rand::rng();
    (0..8)
        .map(|_| CHARS[rng.random_range(0..CHARS.len())] as char)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc::unbounded_channel;

    #[tokio::test]
    async fn create_room_registers_host_and_returns_code() {
        let registry = Registry::new();
        let (tx, _rx) = unbounded_channel();

        let (room_code, host_id) = registry.create_room(tx);

        assert_eq!(room_code.len(), 8);
        assert!(!host_id.is_empty());
    }

    #[tokio::test]
    async fn join_room_returns_none_for_unknown_code() {
        let registry = Registry::new();
        let (tx, _rx) = unbounded_channel();

        assert!(registry.join_room("NOPE0000", tx).is_none());
    }

    #[tokio::test]
    async fn join_room_notifies_host_with_new_peer_id() {
        let registry = Registry::new();
        let (host_tx, mut host_rx) = unbounded_channel();
        let (viewer_tx, _viewer_rx) = unbounded_channel();

        let (room_code, host_id) = registry.create_room(host_tx);
        let (viewer_id, returned_host_id) = registry.join_room(&room_code, viewer_tx).unwrap();

        assert_eq!(returned_host_id, host_id);
        let notification = host_rx.recv().await.unwrap();
        assert_eq!(notification, ServerMessage::PeerJoined { peer_id: viewer_id });
    }

    #[tokio::test]
    async fn relay_sends_only_to_target_peer() {
        let registry = Registry::new();
        let (host_tx, mut host_rx) = unbounded_channel();
        let (viewer_tx, mut viewer_rx) = unbounded_channel();

        let (room_code, host_id) = registry.create_room(host_tx);
        let (viewer_id, _) = registry.join_room(&room_code, viewer_tx).unwrap();
        host_rx.recv().await.unwrap(); // drena o PeerJoined

        registry.relay(
            &room_code,
            &viewer_id,
            ServerMessage::Offer { from: host_id.clone(), sdp: "sdp-data".to_string() },
        );

        let received = viewer_rx.recv().await.unwrap();
        assert_eq!(received, ServerMessage::Offer { from: host_id, sdp: "sdp-data".to_string() });
        assert!(host_rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn leave_room_as_viewer_notifies_remaining_peers() {
        let registry = Registry::new();
        let (host_tx, mut host_rx) = unbounded_channel();
        let (viewer_tx, _viewer_rx) = unbounded_channel();

        let (room_code, _host_id) = registry.create_room(host_tx);
        let (viewer_id, _) = registry.join_room(&room_code, viewer_tx).unwrap();
        host_rx.recv().await.unwrap(); // drena o PeerJoined

        registry.leave_room(&room_code, &viewer_id);

        let notification = host_rx.recv().await.unwrap();
        assert_eq!(notification, ServerMessage::PeerLeft { peer_id: viewer_id });
    }

    #[tokio::test]
    async fn leave_room_as_host_closes_room_and_notifies_viewers() {
        let registry = Registry::new();
        let (host_tx, _host_rx) = unbounded_channel();
        let (viewer_tx, mut viewer_rx) = unbounded_channel();

        let (room_code, host_id) = registry.create_room(host_tx);
        let (_viewer_id, _) = registry.join_room(&room_code, viewer_tx).unwrap();

        registry.leave_room(&room_code, &host_id);

        let notification = viewer_rx.recv().await.unwrap();
        assert_eq!(notification, ServerMessage::RoomClosed);

        assert!(registry.join_room(&room_code, unbounded_channel().0).is_none());
    }
}
