use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rand::RngExt;
use tokio::sync::mpsc::UnboundedSender;
use uuid::Uuid;

use super::auth::{hash_password, verify_password};
use super::protocol::{MemberInfo, ServerMessage, WatcherInfo, MAX_MEMBERS};

/// How long a room stays reservable after its last member leaves before it's
/// actually deleted — long enough to survive a page reload (the old
/// WebSocket drops and a fresh `JoinRoom` arrives once the browser
/// reconnects) without losing the room code to someone else or a "room not
/// found" error.
const EMPTY_ROOM_GRACE_PERIOD: Duration = Duration::from_secs(30);

struct Member {
    nick: String,
    color: String,
    device_id: String,
    sender: UnboundedSender<ServerMessage>,
}

struct Room {
    /// `None` means the room has no password — anyone with the link/code
    /// can join.
    password_hash: Option<String>,
    name: String,
    members: HashMap<String, Member>,
    sharers: HashSet<String>,
    /// sharer_id -> the set of peer_ids currently watching that sharer.
    watchers: HashMap<String, HashSet<String>>,
}

#[derive(Debug)]
pub struct JoinedSnapshot {
    pub peer_id: String,
    pub room_name: String,
    pub members: Vec<MemberInfo>,
    pub active_sharers: Vec<String>,
    pub watcher_info: Vec<WatcherInfo>,
}

pub struct RoomSummary {
    pub name: String,
    pub member_count: usize,
    pub requires_password: bool,
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

    /// Locks the room table. No code in this module panics while holding the
    /// lock (message sends only ever return a `Result` that's discarded), so
    /// the mutex can never actually be poisoned — the `expect` just documents
    /// that assumption instead of unwrapping it silently at nine call sites.
    fn lock_rooms(&self) -> std::sync::MutexGuard<'_, HashMap<String, Room>> {
        self.rooms.lock().expect("room registry mutex should never be poisoned")
    }

    pub fn create_room(
        &self,
        nick: String,
        color: String,
        room_name: String,
        password: Option<String>,
        device_id: String,
        sender: UnboundedSender<ServerMessage>,
    ) -> (String, JoinedSnapshot) {
        let room_code = generate_room_code();
        let peer_id = Uuid::new_v4().to_string();
        let password_hash = hash_optional_password(password);

        let mut members = HashMap::new();
        members.insert(peer_id.clone(), Member { nick: nick.clone(), color: color.clone(), device_id, sender });

        let mut rooms = self.lock_rooms();
        rooms.insert(
            room_code.clone(),
            Room { password_hash, name: room_name.clone(), members, sharers: HashSet::new(), watchers: HashMap::new() },
        );

        let snapshot = JoinedSnapshot {
            peer_id: peer_id.clone(),
            room_name,
            members: vec![MemberInfo { peer_id, nick, color }],
            active_sharers: vec![],
            watcher_info: vec![],
        };
        (room_code, snapshot)
    }

    pub fn join_room(
        &self,
        room_code: &str,
        nick: String,
        color: String,
        password: Option<String>,
        device_id: String,
        sender: UnboundedSender<ServerMessage>,
    ) -> Result<JoinedSnapshot, JoinError> {
        let mut rooms = self.lock_rooms();
        let room = rooms.get_mut(room_code).ok_or(JoinError::NotFound)?;

        if !check_optional_password(password.as_deref(), &room.password_hash) {
            return Err(JoinError::WrongPassword);
        }

        // Same device_id already has an open entry in this room (another tab) —
        // disconnect it before checking capacity, otherwise re-joining from the
        // same device would count as one extra member instead of taking its place.
        if let Some(previous_peer_id) =
            room.members.iter().find(|(_, m)| m.device_id == device_id).map(|(id, _)| id.clone())
        {
            if let Some(removed) = remove_member(room, &previous_peer_id) {
                let _ = removed.sender.send(ServerMessage::Kicked);
            }
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

        room.members.insert(peer_id.clone(), Member { nick: nick.clone(), color: color.clone(), device_id, sender });

        let members: Vec<MemberInfo> = room
            .members
            .iter()
            .map(|(id, m)| MemberInfo { peer_id: id.clone(), nick: m.nick.clone(), color: m.color.clone() })
            .collect();
        let active_sharers: Vec<String> = room.sharers.iter().cloned().collect();
        let watcher_info: Vec<WatcherInfo> = room
            .sharers
            .iter()
            .map(|sharer_id| WatcherInfo {
                sharer_id: sharer_id.clone(),
                watchers: room.watchers.get(sharer_id).map(|w| w.iter().cloned().collect()).unwrap_or_default(),
            })
            .collect();

        Ok(JoinedSnapshot { peer_id, room_name: room.name.clone(), members, active_sharers, watcher_info })
    }

    pub fn room_status(&self, room_code: &str) -> Option<RoomSummary> {
        let rooms = self.lock_rooms();
        rooms.get(room_code).map(|room| RoomSummary {
            name: room.name.clone(),
            member_count: room.members.len(),
            requires_password: room.password_hash.is_some(),
        })
    }

    pub fn start_share(&self, room_code: &str, peer_id: &str) {
        let mut rooms = self.lock_rooms();
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
        let mut rooms = self.lock_rooms();
        if let Some(room) = rooms.get_mut(room_code) {
            room.sharers.remove(peer_id);
            room.watchers.remove(peer_id);
            for (id, member) in room.members.iter() {
                if id != peer_id {
                    let _ = member.sender.send(ServerMessage::PeerStoppedSharing { peer_id: peer_id.to_string() });
                }
            }
        }
    }

    pub fn add_watcher(&self, room_code: &str, sharer_id: &str, viewer_id: &str) {
        let mut rooms = self.lock_rooms();
        let Some(room) = rooms.get_mut(room_code) else { return };

        room.watchers.entry(sharer_id.to_string()).or_default().insert(viewer_id.to_string());
        if let Some(sharer) = room.members.get(sharer_id) {
            let _ = sharer.sender.send(ServerMessage::WatchRequested { from: viewer_id.to_string() });
        }
        broadcast_watchers_changed(room, sharer_id);
    }

    pub fn remove_watcher(&self, room_code: &str, sharer_id: &str, viewer_id: &str) {
        let mut rooms = self.lock_rooms();
        let Some(room) = rooms.get_mut(room_code) else { return };

        if let Some(watchers) = room.watchers.get_mut(sharer_id) {
            watchers.remove(viewer_id);
        }
        if let Some(sharer) = room.members.get(sharer_id) {
            let _ = sharer.sender.send(ServerMessage::WatchStopped { from: viewer_id.to_string() });
        }
        broadcast_watchers_changed(room, sharer_id);
    }

    pub fn relay(&self, room_code: &str, to: &str, message: ServerMessage) {
        let rooms = self.lock_rooms();
        if let Some(room) = rooms.get(room_code) {
            if let Some(member) = room.members.get(to) {
                let _ = member.sender.send(message);
            }
        }
    }

    /// Doesn't delete an emptied room immediately — schedules it for removal
    /// after `EMPTY_ROOM_GRACE_PERIOD` instead, so the same room code stays
    /// joinable if whoever just left (e.g. reloading the page) reconnects.
    pub fn leave_room(&self, room_code: &str, peer_id: &str) {
        let mut rooms = self.lock_rooms();
        let became_empty = if let Some(room) = rooms.get_mut(room_code) {
            remove_member(room, peer_id);
            room.members.is_empty()
        } else {
            false
        };
        drop(rooms);

        if became_empty {
            self.schedule_empty_room_cleanup(room_code.to_string());
        }
    }

    fn schedule_empty_room_cleanup(&self, room_code: String) {
        let registry = self.clone();
        tokio::spawn(async move {
            tokio::time::sleep(EMPTY_ROOM_GRACE_PERIOD).await;
            let mut rooms = registry.lock_rooms();
            // Someone may have rejoined during the grace period — only
            // remove the room if it's *still* empty.
            if rooms.get(&room_code).is_some_and(|room| room.members.is_empty()) {
                rooms.remove(&room_code);
            }
        });
    }
}

/// `None` or an empty string both mean "no password".
fn hash_optional_password(password: Option<String>) -> Option<String> {
    let password = password?;
    (!password.is_empty()).then(|| hash_password(&password))
}

/// A room with no password accepts any join attempt; one with a password
/// requires a matching, non-empty input.
fn check_optional_password(input: Option<&str>, hash: &Option<String>) -> bool {
    match hash {
        None => true,
        Some(hash) => input.is_some_and(|password| !password.is_empty() && verify_password(password, hash)),
    }
}

fn remove_member(room: &mut Room, peer_id: &str) -> Option<Member> {
    let removed = room.members.remove(peer_id)?;
    let was_sharing = room.sharers.remove(peer_id);
    room.watchers.remove(peer_id);
    let affected_sharers: Vec<String> = room
        .watchers
        .iter_mut()
        .filter_map(|(sharer_id, watchers)| watchers.remove(peer_id).then(|| sharer_id.clone()))
        .collect();

    for member in room.members.values() {
        let _ = member.sender.send(ServerMessage::PeerLeft { peer_id: peer_id.to_string() });
        if was_sharing {
            let _ = member.sender.send(ServerMessage::PeerStoppedSharing { peer_id: peer_id.to_string() });
        }
    }
    for sharer_id in &affected_sharers {
        broadcast_watchers_changed(room, sharer_id);
    }

    Some(removed)
}

fn broadcast_watchers_changed(room: &Room, sharer_id: &str) {
    let watchers: Vec<String> = room.watchers.get(sharer_id).map(|w| w.iter().cloned().collect()).unwrap_or_default();
    for member in room.members.values() {
        let _ = member
            .sender
            .send(ServerMessage::WatchersChanged { sharer_id: sharer_id.to_string(), watchers: watchers.clone() });
    }
}

/// Excludes visually ambiguous characters (`I`, `O`, `0`, `1`) so a code
/// read aloud or typed by hand doesn't get misheard/mistyped.
const ROOM_CODE_ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
const ROOM_CODE_LENGTH: usize = 8;

fn generate_room_code() -> String {
    let mut rng = rand::rng();
    (0..ROOM_CODE_LENGTH)
        .map(|_| ROOM_CODE_ALPHABET[rng.random_range(0..ROOM_CODE_ALPHABET.len())] as char)
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

        let (room_code, snapshot) = registry.create_room("Ana".to_string(), "coral".to_string(), "Sala da Ana".to_string(), Some("senha123".to_string()), "device-tx".to_string(), tx);

        assert_eq!(room_code.len(), ROOM_CODE_LENGTH);
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

        let result = registry.join_room("NOPE0000", "Bia".to_string(), "sky".to_string(), Some("senha123".to_string()), "device-tx".to_string(), tx);
        assert_eq!(result.unwrap_err(), JoinError::NotFound);
    }

    #[tokio::test]
    async fn join_room_with_wrong_password_returns_error() {
        let registry = Registry::new();
        let (host_tx, _host_rx) = unbounded_channel();
        let (viewer_tx, _viewer_rx) = unbounded_channel();

        let (room_code, _snapshot) = registry.create_room("Ana".to_string(), "coral".to_string(), "Sala da Ana".to_string(), Some("senha123".to_string()), "device-host".to_string(), host_tx);
        let result = registry.join_room(&room_code, "Bia".to_string(), "sky".to_string(), Some("senha-errada".to_string()), "device-viewer".to_string(), viewer_tx);

        assert_eq!(result.unwrap_err(), JoinError::WrongPassword);
    }

    #[tokio::test]
    async fn join_room_success_notifies_existing_members_and_includes_them_in_snapshot() {
        let registry = Registry::new();
        let (host_tx, mut host_rx) = unbounded_channel();
        let (viewer_tx, _viewer_rx) = unbounded_channel();

        let (room_code, creator_snapshot) = registry.create_room("Ana".to_string(), "coral".to_string(), "Sala da Ana".to_string(), Some("senha123".to_string()), "device-host".to_string(), host_tx);
        let snapshot = registry.join_room(&room_code, "Bia".to_string(), "sky".to_string(), Some("senha123".to_string()), "device-viewer".to_string(), viewer_tx).unwrap();

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
        let (room_code, _snapshot) = registry.create_room("Membro0".to_string(), "coral".to_string(), "Sala da Membro0".to_string(), Some("senha123".to_string()), "device-host".to_string(), host_tx);

        for i in 1..MAX_MEMBERS {
            let (tx, _rx) = unbounded_channel();
            registry
                .join_room(&room_code, format!("Membro{i}"), "sky".to_string(), Some("senha123".to_string()), format!("device-membro-{i}"), tx)
                .expect("deveria caber até MAX_MEMBERS");
        }

        let (extra_tx, _extra_rx) = unbounded_channel();
        let result = registry.join_room(&room_code, "MembroExtra".to_string(), "sky".to_string(), Some("senha123".to_string()), "device-extra".to_string(), extra_tx);
        assert_eq!(result.unwrap_err(), JoinError::Full);
    }

    #[tokio::test]
    async fn join_room_from_same_device_kicks_the_previous_connection() {
        let registry = Registry::new();
        let (host_tx, mut host_rx) = unbounded_channel();
        let (viewer_tx, mut viewer_rx) = unbounded_channel();
        let (room_code, creator_snapshot) =
            registry.create_room("Ana".to_string(), "coral".to_string(), "Sala da Ana".to_string(), Some("senha123".to_string()), "device-ana".to_string(), host_tx);
        registry.join_room(&room_code, "Bia".to_string(), "sky".to_string(), Some("senha123".to_string()), "device-viewer".to_string(), viewer_tx).unwrap();
        host_rx.recv().await.unwrap(); // drain Bia's PeerJoined

        let (host_tx_2, mut host_rx_2) = unbounded_channel();
        let snapshot = registry
            .join_room(&room_code, "AnaCelular".to_string(), "coral".to_string(), Some("senha123".to_string()), "device-ana".to_string(), host_tx_2)
            .unwrap();

        assert_eq!(host_rx.recv().await.unwrap(), ServerMessage::Kicked);

        assert_eq!(viewer_rx.recv().await.unwrap(), ServerMessage::PeerLeft { peer_id: creator_snapshot.peer_id.clone() });
        assert_eq!(
            viewer_rx.recv().await.unwrap(),
            ServerMessage::PeerJoined { peer_id: snapshot.peer_id.clone(), nick: "AnaCelular".to_string(), color: "coral".to_string() }
        );

        assert_eq!(snapshot.members.len(), 2);
        assert!(snapshot.members.iter().any(|m| m.peer_id == snapshot.peer_id && m.nick == "AnaCelular"));
        assert!(!snapshot.members.iter().any(|m| m.peer_id == creator_snapshot.peer_id));

        assert!(host_rx_2.try_recv().is_err());
    }

    #[tokio::test]
    async fn start_share_notifies_others_but_not_self() {
        let registry = Registry::new();
        let (host_tx, mut host_rx) = unbounded_channel();
        let (viewer_tx, mut viewer_rx) = unbounded_channel();

        let (room_code, creator_snapshot) = registry.create_room("Ana".to_string(), "coral".to_string(), "Sala da Ana".to_string(), Some("senha123".to_string()), "device-host".to_string(), host_tx);
        registry.join_room(&room_code, "Bia".to_string(), "sky".to_string(), Some("senha123".to_string()), "device-viewer".to_string(), viewer_tx).unwrap();
        host_rx.recv().await.unwrap(); // drain the PeerJoined

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

        let (room_code, creator_snapshot) = registry.create_room("Ana".to_string(), "coral".to_string(), "Sala da Ana".to_string(), Some("senha123".to_string()), "device-host".to_string(), host_tx);
        registry.join_room(&room_code, "Bia".to_string(), "sky".to_string(), Some("senha123".to_string()), "device-viewer".to_string(), viewer_tx).unwrap();
        host_rx.recv().await.unwrap(); // drain the PeerJoined

        registry.start_share(&room_code, &creator_snapshot.peer_id);
        viewer_rx.recv().await.unwrap(); // drain the PeerStartedSharing

        registry.stop_share(&room_code, &creator_snapshot.peer_id);
        let notification = viewer_rx.recv().await.unwrap();
        assert_eq!(notification, ServerMessage::PeerStoppedSharing { peer_id: creator_snapshot.peer_id });
    }

    #[tokio::test]
    async fn leave_room_survives_when_members_remain() {
        let registry = Registry::new();
        let (host_tx, _host_rx) = unbounded_channel();
        let (viewer_tx, mut viewer_rx) = unbounded_channel();

        let (room_code, creator_snapshot) = registry.create_room("Ana".to_string(), "coral".to_string(), "Sala da Ana".to_string(), Some("senha123".to_string()), "device-host".to_string(), host_tx);
        registry.join_room(&room_code, "Bia".to_string(), "sky".to_string(), Some("senha123".to_string()), "device-viewer".to_string(), viewer_tx).unwrap();

        registry.leave_room(&room_code, &creator_snapshot.peer_id);

        let notification = viewer_rx.recv().await.unwrap();
        assert_eq!(notification, ServerMessage::PeerLeft { peer_id: creator_snapshot.peer_id });

        let (another_tx, _another_rx) = unbounded_channel();
        assert!(registry.join_room(&room_code, "Caio".to_string(), "sky".to_string(), Some("senha123".to_string()), "device-another".to_string(), another_tx).is_ok());
    }

    #[tokio::test]
    async fn leave_room_keeps_an_emptied_room_joinable_during_the_grace_period() {
        let registry = Registry::new();
        let (host_tx, _host_rx) = unbounded_channel();
        let (room_code, creator_snapshot) = registry.create_room("Ana".to_string(), "coral".to_string(), "Sala da Ana".to_string(), Some("senha123".to_string()), "device-host".to_string(), host_tx);

        registry.leave_room(&room_code, &creator_snapshot.peer_id);

        let (tx, _rx) = unbounded_channel();
        let result = registry.join_room(&room_code, "Bia".to_string(), "sky".to_string(), Some("senha123".to_string()), "device-tx".to_string(), tx);
        assert!(result.is_ok(), "an emptied room should still be joinable right away");
    }

    #[tokio::test(start_paused = true)]
    async fn leave_room_removes_an_emptied_room_once_the_grace_period_elapses() {
        let registry = Registry::new();
        let (host_tx, _host_rx) = unbounded_channel();
        let (room_code, creator_snapshot) = registry.create_room("Ana".to_string(), "coral".to_string(), "Sala da Ana".to_string(), Some("senha123".to_string()), "device-host".to_string(), host_tx);

        registry.leave_room(&room_code, &creator_snapshot.peer_id);
        // `leave_room` only *schedules* the cleanup task — give the
        // single-threaded test runtime a turn to actually start it (so its
        // `sleep` registers against the current time) before advancing the
        // clock past the grace period, then again to let it finish running.
        tokio::task::yield_now().await;
        tokio::time::advance(EMPTY_ROOM_GRACE_PERIOD + Duration::from_secs(1)).await;
        tokio::task::yield_now().await;

        let (tx, _rx) = unbounded_channel();
        let result = registry.join_room(&room_code, "Bia".to_string(), "sky".to_string(), Some("senha123".to_string()), "device-tx".to_string(), tx);
        assert_eq!(result.unwrap_err(), JoinError::NotFound);
    }

    #[tokio::test(start_paused = true)]
    async fn leave_room_does_not_remove_the_room_if_someone_rejoins_during_the_grace_period() {
        let registry = Registry::new();
        let (host_tx, _host_rx) = unbounded_channel();
        let (room_code, creator_snapshot) = registry.create_room("Ana".to_string(), "coral".to_string(), "Sala da Ana".to_string(), Some("senha123".to_string()), "device-host".to_string(), host_tx);

        registry.leave_room(&room_code, &creator_snapshot.peer_id);

        let (tx, _rx) = unbounded_channel();
        registry.join_room(&room_code, "Bia".to_string(), "sky".to_string(), Some("senha123".to_string()), "device-tx".to_string(), tx).unwrap();

        tokio::time::advance(EMPTY_ROOM_GRACE_PERIOD + Duration::from_secs(1)).await;

        let status = registry.room_status(&room_code);
        assert!(status.is_some(), "the room should survive since it's no longer empty");
    }

    #[tokio::test]
    async fn leave_room_while_sharing_also_sends_peer_stopped_sharing() {
        let registry = Registry::new();
        let (host_tx, _host_rx) = unbounded_channel();
        let (viewer_tx, mut viewer_rx) = unbounded_channel();

        let (room_code, creator_snapshot) = registry.create_room("Ana".to_string(), "coral".to_string(), "Sala da Ana".to_string(), Some("senha123".to_string()), "device-host".to_string(), host_tx);
        registry.join_room(&room_code, "Bia".to_string(), "sky".to_string(), Some("senha123".to_string()), "device-viewer".to_string(), viewer_tx).unwrap();
        registry.start_share(&room_code, &creator_snapshot.peer_id);
        viewer_rx.recv().await.unwrap(); // drain the PeerStartedSharing

        registry.leave_room(&room_code, &creator_snapshot.peer_id);

        let left = viewer_rx.recv().await.unwrap();
        assert_eq!(left, ServerMessage::PeerLeft { peer_id: creator_snapshot.peer_id.clone() });
        let stopped = viewer_rx.recv().await.unwrap();
        assert_eq!(stopped, ServerMessage::PeerStoppedSharing { peer_id: creator_snapshot.peer_id });
    }

    #[tokio::test]
    async fn add_watcher_notifies_sharer_and_broadcasts_count_to_everyone() {
        let registry = Registry::new();
        let (host_tx, mut host_rx) = unbounded_channel();
        let (viewer_tx, mut viewer_rx) = unbounded_channel();

        let (room_code, creator_snapshot) = registry.create_room("Ana".to_string(), "coral".to_string(), "Sala da Ana".to_string(), Some("senha123".to_string()), "device-host".to_string(), host_tx);
        let viewer_snapshot = registry.join_room(&room_code, "Bia".to_string(), "sky".to_string(), Some("senha123".to_string()), "device-viewer".to_string(), viewer_tx).unwrap();
        host_rx.recv().await.unwrap(); // drain the PeerJoined

        registry.add_watcher(&room_code, &creator_snapshot.peer_id, &viewer_snapshot.peer_id);

        assert_eq!(host_rx.recv().await.unwrap(), ServerMessage::WatchRequested { from: viewer_snapshot.peer_id.clone() });
        assert_eq!(
            host_rx.recv().await.unwrap(),
            ServerMessage::WatchersChanged { sharer_id: creator_snapshot.peer_id.clone(), watchers: vec![viewer_snapshot.peer_id.clone()] }
        );
        assert_eq!(
            viewer_rx.recv().await.unwrap(),
            ServerMessage::WatchersChanged { sharer_id: creator_snapshot.peer_id, watchers: vec![viewer_snapshot.peer_id] }
        );
    }

    #[tokio::test]
    async fn remove_watcher_notifies_sharer_and_broadcasts_updated_count() {
        let registry = Registry::new();
        let (host_tx, mut host_rx) = unbounded_channel();
        let (viewer_tx, mut viewer_rx) = unbounded_channel();

        let (room_code, creator_snapshot) = registry.create_room("Ana".to_string(), "coral".to_string(), "Sala da Ana".to_string(), Some("senha123".to_string()), "device-host".to_string(), host_tx);
        let viewer_snapshot = registry.join_room(&room_code, "Bia".to_string(), "sky".to_string(), Some("senha123".to_string()), "device-viewer".to_string(), viewer_tx).unwrap();
        host_rx.recv().await.unwrap(); // drain the PeerJoined

        registry.add_watcher(&room_code, &creator_snapshot.peer_id, &viewer_snapshot.peer_id);
        host_rx.recv().await.unwrap(); // drain the WatchRequested
        host_rx.recv().await.unwrap(); // drain the WatchersChanged
        viewer_rx.recv().await.unwrap(); // drain the WatchersChanged

        registry.remove_watcher(&room_code, &creator_snapshot.peer_id, &viewer_snapshot.peer_id);

        assert_eq!(host_rx.recv().await.unwrap(), ServerMessage::WatchStopped { from: viewer_snapshot.peer_id.clone() });
        assert_eq!(
            host_rx.recv().await.unwrap(),
            ServerMessage::WatchersChanged { sharer_id: creator_snapshot.peer_id.clone(), watchers: vec![] }
        );
        assert_eq!(
            viewer_rx.recv().await.unwrap(),
            ServerMessage::WatchersChanged { sharer_id: creator_snapshot.peer_id, watchers: vec![] }
        );
    }

    #[tokio::test]
    async fn join_room_snapshot_includes_watcher_info_for_active_sharers() {
        let registry = Registry::new();
        let (host_tx, _host_rx) = unbounded_channel();
        let (viewer_tx, mut viewer_rx) = unbounded_channel();

        let (room_code, creator_snapshot) = registry.create_room("Ana".to_string(), "coral".to_string(), "Sala da Ana".to_string(), Some("senha123".to_string()), "device-host".to_string(), host_tx);
        let viewer_snapshot = registry.join_room(&room_code, "Bia".to_string(), "sky".to_string(), Some("senha123".to_string()), "device-viewer".to_string(), viewer_tx).unwrap();
        registry.start_share(&room_code, &creator_snapshot.peer_id);
        registry.add_watcher(&room_code, &creator_snapshot.peer_id, &viewer_snapshot.peer_id);
        viewer_rx.recv().await.unwrap(); // drain PeerStartedSharing
        viewer_rx.recv().await.unwrap(); // drain WatchersChanged

        let (late_tx, _late_rx) = unbounded_channel();
        let late_snapshot = registry.join_room(&room_code, "Caio".to_string(), "sky".to_string(), Some("senha123".to_string()), "device-late".to_string(), late_tx).unwrap();

        assert_eq!(
            late_snapshot.watcher_info,
            vec![WatcherInfo { sharer_id: creator_snapshot.peer_id, watchers: vec![viewer_snapshot.peer_id] }]
        );
    }

    #[tokio::test]
    async fn leave_room_removes_the_leaver_from_watcher_lists_and_broadcasts_update() {
        let registry = Registry::new();
        let (host_tx, mut host_rx) = unbounded_channel();
        let (viewer_tx, _viewer_rx) = unbounded_channel();

        let (room_code, creator_snapshot) = registry.create_room("Ana".to_string(), "coral".to_string(), "Sala da Ana".to_string(), Some("senha123".to_string()), "device-host".to_string(), host_tx);
        let viewer_snapshot = registry.join_room(&room_code, "Bia".to_string(), "sky".to_string(), Some("senha123".to_string()), "device-viewer".to_string(), viewer_tx).unwrap();
        host_rx.recv().await.unwrap(); // drain PeerJoined

        registry.add_watcher(&room_code, &creator_snapshot.peer_id, &viewer_snapshot.peer_id);
        host_rx.recv().await.unwrap(); // drain WatchRequested
        host_rx.recv().await.unwrap(); // drain WatchersChanged

        registry.leave_room(&room_code, &viewer_snapshot.peer_id);

        assert_eq!(host_rx.recv().await.unwrap(), ServerMessage::PeerLeft { peer_id: viewer_snapshot.peer_id.clone() });
        assert_eq!(
            host_rx.recv().await.unwrap(),
            ServerMessage::WatchersChanged { sharer_id: creator_snapshot.peer_id, watchers: vec![] }
        );
    }

    #[tokio::test]
    async fn room_status_reports_name_and_member_count_for_existing_room() {
        let registry = Registry::new();
        let (host_tx, _host_rx) = unbounded_channel();
        let (room_code, _snapshot) =
            registry.create_room("Ana".to_string(), "coral".to_string(), "Sala da Ana".to_string(), Some("senha123".to_string()), "device-host".to_string(), host_tx);

        let (viewer_tx, _viewer_rx) = unbounded_channel();
        registry.join_room(&room_code, "Bia".to_string(), "sky".to_string(), Some("senha123".to_string()), "device-viewer".to_string(), viewer_tx).unwrap();

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
        let (host_tx, _host_rx) = unbounded_channel();
        let (room_code, _snapshot) =
            registry.create_room("Ana".to_string(), "coral".to_string(), "Sala da Ana".to_string(), None, "device-host".to_string(), host_tx);

        assert!(!registry.room_status(&room_code).unwrap().requires_password);

        let (viewer_tx, _viewer_rx) = unbounded_channel();
        let result = registry.join_room(&room_code, "Bia".to_string(), "sky".to_string(), None, "device-viewer".to_string(), viewer_tx);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn create_room_with_an_empty_password_behaves_as_no_password() {
        let registry = Registry::new();
        let (host_tx, _host_rx) = unbounded_channel();
        let (room_code, _snapshot) = registry.create_room(
            "Ana".to_string(),
            "coral".to_string(),
            "Sala da Ana".to_string(),
            Some(String::new()),
            "device-host".to_string(),
            host_tx,
        );

        assert!(!registry.room_status(&room_code).unwrap().requires_password);
    }

    #[tokio::test]
    async fn join_room_with_a_password_fails_without_one_when_the_room_requires_it() {
        let registry = Registry::new();
        let (host_tx, _host_rx) = unbounded_channel();
        let (room_code, _snapshot) = registry.create_room("Ana".to_string(), "coral".to_string(), "Sala da Ana".to_string(), Some("senha123".to_string()), "device-host".to_string(), host_tx);

        let (viewer_tx, _viewer_rx) = unbounded_channel();
        let result = registry.join_room(&room_code, "Bia".to_string(), "sky".to_string(), None, "device-viewer".to_string(), viewer_tx);
        assert_eq!(result.unwrap_err(), JoinError::WrongPassword);
    }
}
