use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use rand::RngExt;
use tokio::sync::mpsc::UnboundedSender;
use uuid::Uuid;

use super::auth::{hash_password, verify_password};
use super::protocol::{MemberInfo, ServerMessage, MAX_MEMBERS};

struct Member {
    nick: String,
    color: String,
    sender: UnboundedSender<ServerMessage>,
}

struct Room {
    password_hash: String,
    name: String,
    members: HashMap<String, Member>,
    sharers: HashSet<String>,
}

#[derive(Debug)]
pub struct JoinedSnapshot {
    pub peer_id: String,
    pub room_name: String,
    pub members: Vec<MemberInfo>,
    pub active_sharers: Vec<String>,
}

pub struct RoomSummary {
    pub name: String,
    pub member_count: usize,
}

#[derive(Debug, PartialEq)]
pub enum JoinError {
    NotFound,
    WrongPassword,
    Full,
}

#[derive(Clone, Default)]
pub struct Registry {
    rooms: Arc<Mutex<HashMap<String, Room>>>,
}

impl Registry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn create_room(
        &self,
        nick: String,
        color: String,
        room_name: String,
        password: &str,
        sender: UnboundedSender<ServerMessage>,
    ) -> (String, JoinedSnapshot) {
        let room_code = generate_room_code();
        let peer_id = Uuid::new_v4().to_string();
        let password_hash = hash_password(password);

        let mut members = HashMap::new();
        members.insert(peer_id.clone(), Member { nick: nick.clone(), color: color.clone(), sender });

        let mut rooms = self.rooms.lock().unwrap();
        rooms.insert(
            room_code.clone(),
            Room { password_hash, name: room_name.clone(), members, sharers: HashSet::new() },
        );

        let snapshot = JoinedSnapshot {
            peer_id: peer_id.clone(),
            room_name,
            members: vec![MemberInfo { peer_id, nick, color }],
            active_sharers: vec![],
        };
        (room_code, snapshot)
    }

    pub fn join_room(
        &self,
        room_code: &str,
        nick: String,
        color: String,
        password: &str,
        sender: UnboundedSender<ServerMessage>,
    ) -> Result<JoinedSnapshot, JoinError> {
        let mut rooms = self.rooms.lock().unwrap();
        let room = rooms.get_mut(room_code).ok_or(JoinError::NotFound)?;

        if !verify_password(password, &room.password_hash) {
            return Err(JoinError::WrongPassword);
        }
        if room.members.len() >= MAX_MEMBERS {
            return Err(JoinError::Full);
        }

        let peer_id = Uuid::new_v4().to_string();

        for member in room.members.values() {
            let _ = member.sender.send(ServerMessage::PeerJoined {
                peer_id: peer_id.clone(),
                nick: nick.clone(),
                color: color.clone(),
            });
        }

        room.members.insert(peer_id.clone(), Member { nick: nick.clone(), color: color.clone(), sender });

        let members: Vec<MemberInfo> = room
            .members
            .iter()
            .map(|(id, m)| MemberInfo { peer_id: id.clone(), nick: m.nick.clone(), color: m.color.clone() })
            .collect();
        let active_sharers: Vec<String> = room.sharers.iter().cloned().collect();

        Ok(JoinedSnapshot { peer_id, room_name: room.name.clone(), members, active_sharers })
    }

    pub fn room_status(&self, room_code: &str) -> Option<RoomSummary> {
        let rooms = self.rooms.lock().unwrap();
        rooms.get(room_code).map(|room| RoomSummary { name: room.name.clone(), member_count: room.members.len() })
    }

    pub fn start_share(&self, room_code: &str, peer_id: &str) {
        let mut rooms = self.rooms.lock().unwrap();
        if let Some(room) = rooms.get_mut(room_code) {
            room.sharers.insert(peer_id.to_string());
            for (id, member) in room.members.iter() {
                if id != peer_id {
                    let _ = member.sender.send(ServerMessage::PeerStartedSharing { peer_id: peer_id.to_string() });
                }
            }
        }
    }

    pub fn stop_share(&self, room_code: &str, peer_id: &str) {
        let mut rooms = self.rooms.lock().unwrap();
        if let Some(room) = rooms.get_mut(room_code) {
            room.sharers.remove(peer_id);
            for (id, member) in room.members.iter() {
                if id != peer_id {
                    let _ = member.sender.send(ServerMessage::PeerStoppedSharing { peer_id: peer_id.to_string() });
                }
            }
        }
    }

    pub fn relay(&self, room_code: &str, to: &str, message: ServerMessage) {
        let rooms = self.rooms.lock().unwrap();
        if let Some(room) = rooms.get(room_code) {
            if let Some(member) = room.members.get(to) {
                let _ = member.sender.send(message);
            }
        }
    }

    pub fn leave_room(&self, room_code: &str, peer_id: &str) {
        let mut rooms = self.rooms.lock().unwrap();
        let mut remove_room = false;

        if let Some(room) = rooms.get_mut(room_code) {
            room.members.remove(peer_id);
            let was_sharing = room.sharers.remove(peer_id);

            for member in room.members.values() {
                let _ = member.sender.send(ServerMessage::PeerLeft { peer_id: peer_id.to_string() });
                if was_sharing {
                    let _ = member
                        .sender
                        .send(ServerMessage::PeerStoppedSharing { peer_id: peer_id.to_string() });
                }
            }

            if room.members.is_empty() {
                remove_room = true;
            }
        }

        if remove_room {
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
    async fn create_room_registers_creator_and_returns_snapshot() {
        let registry = Registry::new();
        let (tx, _rx) = unbounded_channel();

        let (room_code, snapshot) = registry.create_room("Ana".to_string(), "coral".to_string(), "Sala da Ana".to_string(), "senha123", tx);

        assert_eq!(room_code.len(), 8);
        assert_eq!(
            snapshot.members,
            vec![MemberInfo { peer_id: snapshot.peer_id.clone(), nick: "Ana".to_string(), color: "coral".to_string() }]
        );
        assert_eq!(snapshot.room_name, "Sala da Ana");
        assert!(snapshot.active_sharers.is_empty());
    }

    #[tokio::test]
    async fn join_room_not_found_returns_error() {
        let registry = Registry::new();
        let (tx, _rx) = unbounded_channel();

        let result = registry.join_room("NOPE0000", "Bia".to_string(), "sky".to_string(), "senha123", tx);
        assert_eq!(result.unwrap_err(), JoinError::NotFound);
    }

    #[tokio::test]
    async fn join_room_with_wrong_password_returns_error() {
        let registry = Registry::new();
        let (host_tx, _host_rx) = unbounded_channel();
        let (viewer_tx, _viewer_rx) = unbounded_channel();

        let (room_code, _snapshot) = registry.create_room("Ana".to_string(), "coral".to_string(), "Sala da Ana".to_string(), "senha123", host_tx);
        let result = registry.join_room(&room_code, "Bia".to_string(), "sky".to_string(), "senha-errada", viewer_tx);

        assert_eq!(result.unwrap_err(), JoinError::WrongPassword);
    }

    #[tokio::test]
    async fn join_room_success_notifies_existing_members_and_includes_them_in_snapshot() {
        let registry = Registry::new();
        let (host_tx, mut host_rx) = unbounded_channel();
        let (viewer_tx, _viewer_rx) = unbounded_channel();

        let (room_code, creator_snapshot) = registry.create_room("Ana".to_string(), "coral".to_string(), "Sala da Ana".to_string(), "senha123", host_tx);
        let snapshot = registry.join_room(&room_code, "Bia".to_string(), "sky".to_string(), "senha123", viewer_tx).unwrap();

        assert_eq!(snapshot.members.len(), 2);
        assert!(snapshot.members.iter().any(|m| m.peer_id == creator_snapshot.peer_id && m.nick == "Ana"));
        assert!(snapshot.members.iter().any(|m| m.peer_id == snapshot.peer_id && m.nick == "Bia"));

        let notification = host_rx.recv().await.unwrap();
        assert_eq!(
            notification,
            ServerMessage::PeerJoined { peer_id: snapshot.peer_id.clone(), nick: "Bia".to_string(), color: "sky".to_string() }
        );
    }

    #[tokio::test]
    async fn join_room_full_returns_error() {
        let registry = Registry::new();
        let (host_tx, _host_rx) = unbounded_channel();
        let (room_code, _snapshot) = registry.create_room("Membro0".to_string(), "coral".to_string(), "Sala da Membro0".to_string(), "senha123", host_tx);

        for i in 1..MAX_MEMBERS {
            let (tx, _rx) = unbounded_channel();
            registry
                .join_room(&room_code, format!("Membro{i}"), "sky".to_string(), "senha123", tx)
                .expect("deveria caber até MAX_MEMBERS");
        }

        let (extra_tx, _extra_rx) = unbounded_channel();
        let result = registry.join_room(&room_code, "MembroExtra".to_string(), "sky".to_string(), "senha123", extra_tx);
        assert_eq!(result.unwrap_err(), JoinError::Full);
    }

    #[tokio::test]
    async fn start_share_notifies_others_but_not_self() {
        let registry = Registry::new();
        let (host_tx, mut host_rx) = unbounded_channel();
        let (viewer_tx, mut viewer_rx) = unbounded_channel();

        let (room_code, creator_snapshot) = registry.create_room("Ana".to_string(), "coral".to_string(), "Sala da Ana".to_string(), "senha123", host_tx);
        registry.join_room(&room_code, "Bia".to_string(), "sky".to_string(), "senha123", viewer_tx).unwrap();
        host_rx.recv().await.unwrap(); // drena o PeerJoined

        registry.start_share(&room_code, &creator_snapshot.peer_id);

        let notification = viewer_rx.recv().await.unwrap();
        assert_eq!(notification, ServerMessage::PeerStartedSharing { peer_id: creator_snapshot.peer_id.clone() });
        assert!(host_rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn stop_share_notifies_others() {
        let registry = Registry::new();
        let (host_tx, mut host_rx) = unbounded_channel();
        let (viewer_tx, mut viewer_rx) = unbounded_channel();

        let (room_code, creator_snapshot) = registry.create_room("Ana".to_string(), "coral".to_string(), "Sala da Ana".to_string(), "senha123", host_tx);
        registry.join_room(&room_code, "Bia".to_string(), "sky".to_string(), "senha123", viewer_tx).unwrap();
        host_rx.recv().await.unwrap(); // drena o PeerJoined

        registry.start_share(&room_code, &creator_snapshot.peer_id);
        viewer_rx.recv().await.unwrap(); // drena o PeerStartedSharing

        registry.stop_share(&room_code, &creator_snapshot.peer_id);
        let notification = viewer_rx.recv().await.unwrap();
        assert_eq!(notification, ServerMessage::PeerStoppedSharing { peer_id: creator_snapshot.peer_id });
    }

    #[tokio::test]
    async fn leave_room_survives_when_members_remain() {
        let registry = Registry::new();
        let (host_tx, _host_rx) = unbounded_channel();
        let (viewer_tx, mut viewer_rx) = unbounded_channel();

        let (room_code, creator_snapshot) = registry.create_room("Ana".to_string(), "coral".to_string(), "Sala da Ana".to_string(), "senha123", host_tx);
        registry.join_room(&room_code, "Bia".to_string(), "sky".to_string(), "senha123", viewer_tx).unwrap();

        registry.leave_room(&room_code, &creator_snapshot.peer_id);

        let notification = viewer_rx.recv().await.unwrap();
        assert_eq!(notification, ServerMessage::PeerLeft { peer_id: creator_snapshot.peer_id });

        // A sala continua existindo — entrar de novo com a senha certa funciona.
        let (another_tx, _another_rx) = unbounded_channel();
        assert!(registry.join_room(&room_code, "Caio".to_string(), "sky".to_string(), "senha123", another_tx).is_ok());
    }

    #[tokio::test]
    async fn leave_room_removes_room_when_last_member_leaves() {
        let registry = Registry::new();
        let (host_tx, _host_rx) = unbounded_channel();
        let (room_code, creator_snapshot) = registry.create_room("Ana".to_string(), "coral".to_string(), "Sala da Ana".to_string(), "senha123", host_tx);

        registry.leave_room(&room_code, &creator_snapshot.peer_id);

        let (tx, _rx) = unbounded_channel();
        let result = registry.join_room(&room_code, "Bia".to_string(), "sky".to_string(), "senha123", tx);
        assert_eq!(result.unwrap_err(), JoinError::NotFound);
    }

    #[tokio::test]
    async fn leave_room_while_sharing_also_sends_peer_stopped_sharing() {
        let registry = Registry::new();
        let (host_tx, _host_rx) = unbounded_channel();
        let (viewer_tx, mut viewer_rx) = unbounded_channel();

        let (room_code, creator_snapshot) = registry.create_room("Ana".to_string(), "coral".to_string(), "Sala da Ana".to_string(), "senha123", host_tx);
        registry.join_room(&room_code, "Bia".to_string(), "sky".to_string(), "senha123", viewer_tx).unwrap();
        registry.start_share(&room_code, &creator_snapshot.peer_id);
        viewer_rx.recv().await.unwrap(); // drena o PeerStartedSharing

        registry.leave_room(&room_code, &creator_snapshot.peer_id);

        let left = viewer_rx.recv().await.unwrap();
        assert_eq!(left, ServerMessage::PeerLeft { peer_id: creator_snapshot.peer_id.clone() });
        let stopped = viewer_rx.recv().await.unwrap();
        assert_eq!(stopped, ServerMessage::PeerStoppedSharing { peer_id: creator_snapshot.peer_id });
    }

    #[tokio::test]
    async fn room_status_reports_name_and_member_count_for_existing_room() {
        let registry = Registry::new();
        let (host_tx, _host_rx) = unbounded_channel();
        let (room_code, _snapshot) =
            registry.create_room("Ana".to_string(), "coral".to_string(), "Sala da Ana".to_string(), "senha123", host_tx);

        let (viewer_tx, _viewer_rx) = unbounded_channel();
        registry.join_room(&room_code, "Bia".to_string(), "sky".to_string(), "senha123", viewer_tx).unwrap();

        let status = registry.room_status(&room_code).unwrap();
        assert_eq!(status.name, "Sala da Ana");
        assert_eq!(status.member_count, 2);
    }

    #[tokio::test]
    async fn room_status_is_none_for_unknown_room() {
        let registry = Registry::new();
        assert!(registry.room_status("NOPE0000").is_none());
    }
}
