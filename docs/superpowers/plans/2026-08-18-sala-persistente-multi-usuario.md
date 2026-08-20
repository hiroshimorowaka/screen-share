# Sala Persistente Multiusuário com Senha — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Transformar o compartilhamento de tela 1-para-N efêmero (v1) numa sala persistente, protegida por senha, onde qualquer membro pode compartilhar sua tela a qualquer momento e todos veem, numa grade, as transmissões ativas simultâneas dos outros.

**Architecture:** Mesmo binário Rust único (Leptos SSR sobre Axum) e mesmo papel do WebSocket `/ws` como canal só de sinalização (nunca carrega vídeo). O conceito de "host" desaparece: o registro de salas em memória passa a rastrear `members` (com nick) e `sharers` (quem está compartilhando agora); qualquer par (sharer → viewer) ganha sua própria `RTCPeerConnection`, sempre com o sharer como quem oferta. A sala só é removida do registro quando o último membro sai.

**Tech Stack:** Rust, Leptos (SSR + hydratação WASM) via `cargo-leptos`, `leptos_router`, Axum, Tokio, `serde`/`serde_json`, `argon2` (hash de senha), `web-sys`/`wasm-bindgen`/`wasm-bindgen-futures` para `getDisplayMedia`, `RTCPeerConnection` e `localStorage` no navegador, `uuid`, `rand`, `tokio-tungstenite` (testes de integração).

## Global Constraints

- Sala identificada por um código gerado pelo sistema (8 caracteres, mesmo formato do v1) e protegida por senha; a senha é guardada como hash `argon2`, nunca em texto puro.
- Todo participante — inclusive quem cria a sala — informa nick + senha antes de entrar. Nick fica salvo em `localStorage`; senha nunca é persistida no navegador.
- Sala só é removida do registro em memória quando o último membro sai; sair de quem criou não afeta a sala. Não sobrevive a um reinício do processo do servidor (sem banco de dados).
- Limite de 8 membros simultâneos por sala (`RoomFull` acima disso).
- Qualquer membro pode iniciar/parar seu próprio compartilhamento a qualquer momento; sem hierarquia entre participantes (sem "dono da sala").
- Sem áudio no compartilhamento, sem controle de volume por espectador, sem TURN/SFU, sem rate limiting de tentativas de senha — todos fora de escopo nesta fase (ver spec).
- Vídeo sempre P2P direto via WebRTC (STUN público); o servidor `/ws` só troca mensagens de sinalização.
- **Nota sobre risco de API:** as chamadas a `web-sys` (WebRTC, `localStorage`, `RtcPeerConnectionIceEvent`, etc.) seguem a convenção de setter/getter da versão do `web-sys` já usada no projeto (`0.3.104`). Se o `cargo build` apontar um método com nome ligeiramente diferente do usado nas tasks abaixo, é só uma mudança de nome entre versões do crate — ajuste conforme o erro do compilador indicar; a lógica ao redor não muda.

---

## Task 1: Hash de senha (`argon2`)

**Files:**
- Create: `src/signaling/auth.rs`
- Modify: `src/signaling/mod.rs`
- Modify: `Cargo.toml` (dependência `argon2`, feature `ssr`)
- Test: incluído como `#[cfg(test)] mod tests` dentro de `src/signaling/auth.rs`

**Interfaces:**
- Produces: `crate::signaling::auth::{hash_password(password: &str) -> String, verify_password(password: &str, hash: &str) -> bool}` — usadas pela Task 3.

- [ ] **Step 1: Adicionar a dependência `argon2`**

```bash
cargo add argon2 --optional
```

Em `Cargo.toml`, garanta que `argon2` está na lista `[dependencies]` com `optional = true` (o `cargo add --optional` já faz isso) e adicione `"dep:argon2"` à lista da feature `ssr`:

```toml
ssr = [
    "dep:axum",
    "dep:tokio",
    "dep:leptos_axum",
    "dep:uuid",
    "dep:rand",
    "dep:futures-util",
    "dep:argon2",
    "leptos/ssr",
    "leptos_meta/ssr",
    "leptos_router/ssr",
]
```

- [ ] **Step 2: Declarar o módulo**

`src/signaling/mod.rs` — adicione junto aos demais módulos `ssr`:

```rust
#[cfg(feature = "ssr")]
pub mod auth;
```

- [ ] **Step 3: Escrever os testes que falham**

`src/signaling/auth.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_password_does_not_store_plaintext() {
        let hash = hash_password("minha-senha-123");
        assert_ne!(hash, "minha-senha-123");
        assert!(!hash.is_empty());
    }

    #[test]
    fn verify_password_accepts_correct_password() {
        let hash = hash_password("minha-senha-123");
        assert!(verify_password("minha-senha-123", &hash));
    }

    #[test]
    fn verify_password_rejects_wrong_password() {
        let hash = hash_password("minha-senha-123");
        assert!(!verify_password("senha-errada", &hash));
    }
}
```

- [ ] **Step 4: Rodar os testes e confirmar que falham**

Run: `cargo test --lib --features ssr signaling::auth`
Expected: FAIL — `hash_password`/`verify_password` não existem.

- [ ] **Step 5: Implementar**

No topo de `src/signaling/auth.rs`, antes do `#[cfg(test)]`:

```rust
use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;

pub fn hash_password(password: &str) -> String {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .expect("hashing de senha não deveria falhar")
        .to_string()
}

pub fn verify_password(password: &str, hash: &str) -> bool {
    let Ok(parsed_hash) = PasswordHash::new(hash) else { return false };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok()
}
```

Se o compilador não encontrar `argon2::password_hash`, rode `cargo add argon2 --optional --features password-hash` — em algumas versões do crate esse submódulo só é reexportado com a feature `password-hash` explícita.

- [ ] **Step 6: Rodar os testes e confirmar que passam**

Run: `cargo test --lib --features ssr signaling::auth`
Expected: PASS (3 testes).

- [ ] **Step 7: Commit**

```bash
git add src/signaling/auth.rs src/signaling/mod.rs Cargo.toml Cargo.lock
git commit -m "feat: add argon2 password hashing helpers"
```

---

## Task 2: Protocolo de sinalização v2

**Files:**
- Modify: `src/signaling/protocol.rs` (substituir os enums `ClientMessage`/`ServerMessage` do v1 e seus testes)

**Interfaces:**
- Produces: `crate::signaling::protocol::{ClientMessage, ServerMessage, MemberInfo}` — usados pelas Tasks 3, 4, 5, 6, 7, 8.

- [ ] **Step 1: Escrever os testes que falham (substituindo os testes do v1)**

Substitua todo o conteúdo de `src/signaling/protocol.rs` por:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MemberInfo {
    pub peer_id: String,
    pub nick: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    CreateRoom { nick: String, password: String },
    JoinRoom { room: String, nick: String, password: String },
    StartShare,
    StopShare,
    Offer { to: String, sdp: String },
    Answer { to: String, sdp: String },
    IceCandidate {
        to: String,
        stream_owner: String,
        candidate: String,
        sdp_mid: Option<String>,
        sdp_m_line_index: Option<u16>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    Joined {
        peer_id: String,
        room: String,
        members: Vec<MemberInfo>,
        active_sharers: Vec<String>,
    },
    AuthFailed,
    RoomNotFound,
    RoomFull,
    PeerJoined { peer_id: String, nick: String },
    PeerLeft { peer_id: String },
    PeerStartedSharing { peer_id: String },
    PeerStoppedSharing { peer_id: String },
    Offer { from: String, sdp: String },
    Answer { from: String, sdp: String },
    IceCandidate {
        from: String,
        stream_owner: String,
        candidate: String,
        sdp_mid: Option<String>,
        sdp_m_line_index: Option<u16>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_room_message_round_trips_through_json() {
        let msg = ClientMessage::CreateRoom { nick: "Ana".to_string(), password: "abacate".to_string() };
        let json = serde_json::to_string(&msg).unwrap();
        assert_eq!(json, r#"{"type":"create_room","nick":"Ana","password":"abacate"}"#);

        let parsed: ClientMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, msg);
    }

    #[test]
    fn join_room_message_round_trips_through_json() {
        let msg = ClientMessage::JoinRoom {
            room: "ABCD1234".to_string(),
            nick: "Bia".to_string(),
            password: "abacate".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: ClientMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, msg);
    }

    #[test]
    fn joined_server_message_round_trips_through_json() {
        let msg = ServerMessage::Joined {
            peer_id: "peer-1".to_string(),
            room: "ABCD1234".to_string(),
            members: vec![MemberInfo { peer_id: "peer-1".to_string(), nick: "Ana".to_string() }],
            active_sharers: vec![],
        };
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: ServerMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, msg);
    }

    #[test]
    fn ice_candidate_carries_stream_owner() {
        let msg = ClientMessage::IceCandidate {
            to: "peer-2".to_string(),
            stream_owner: "peer-1".to_string(),
            candidate: "candidate-data".to_string(),
            sdp_mid: None,
            sdp_m_line_index: None,
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains(r#""stream_owner":"peer-1""#));

        let parsed: ClientMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, msg);
    }
}
```

- [ ] **Step 2: Rodar os testes e confirmar que falham**

Run: `cargo test --lib signaling::protocol`
Expected: FAIL — os testes antigos (`join_message_round_trips_through_json`, `offer_server_message_round_trips_through_json`) referenciavam variantes que não existem mais; os novos ainda não existiam antes deste passo, então essa substituição já deixa o arquivo no estado final. Confirme que a compilação falha antes deste step e passa depois (não há um "meio termo" aqui porque estamos substituindo o arquivo inteiro).

- [ ] **Step 3: Rodar os testes e confirmar que passam**

Run: `cargo test --lib signaling::protocol`
Expected: PASS (4 testes).

- [ ] **Step 4: Commit**

```bash
git add src/signaling/protocol.rs
git commit -m "feat: redesign signaling protocol for multi-sharer authenticated rooms"
```

---

## Task 3: Registro de salas v2 (membros, senha, sharers, capacidade)

**Files:**
- Modify: `src/signaling/registry.rs` (substituir por completo o `Registry` do v1)

**Interfaces:**
- Consumes: `crate::signaling::auth::{hash_password, verify_password}` (Task 1), `crate::signaling::protocol::{MemberInfo, ServerMessage}` (Task 2).
- Produces: `crate::signaling::registry::{Registry, JoinedSnapshot, JoinError, MAX_MEMBERS}`:
  - `Registry::new() -> Self`
  - `Registry::create_room(&self, nick: String, password: &str, sender: UnboundedSender<ServerMessage>) -> (String, JoinedSnapshot)` — retorna `(room_code, snapshot)`.
  - `Registry::join_room(&self, room_code: &str, nick: String, password: &str, sender: UnboundedSender<ServerMessage>) -> Result<JoinedSnapshot, JoinError>`.
  - `Registry::start_share(&self, room_code: &str, peer_id: &str)`, `Registry::stop_share(&self, room_code: &str, peer_id: &str)`.
  - `Registry::relay(&self, room_code: &str, to: &str, message: ServerMessage)`.
  - `Registry::leave_room(&self, room_code: &str, peer_id: &str)`.
  - `JoinedSnapshot { peer_id: String, members: Vec<MemberInfo>, active_sharers: Vec<String> }`.
  - `JoinError { NotFound, WrongPassword, Full }` (`Debug + PartialEq`).

- [ ] **Step 1: Escrever os testes que falham (substituindo os testes do v1)**

Substitua todo o conteúdo de `src/signaling/registry.rs` por (implementação incluída já neste passo — ver nota abaixo sobre por que registry e seus testes vêm juntos):

```rust
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use rand::RngExt;
use tokio::sync::mpsc::UnboundedSender;
use uuid::Uuid;

use super::auth::{hash_password, verify_password};
use super::protocol::{MemberInfo, ServerMessage};

pub const MAX_MEMBERS: usize = 8;

struct Member {
    nick: String,
    sender: UnboundedSender<ServerMessage>,
}

struct Room {
    password_hash: String,
    members: HashMap<String, Member>,
    sharers: HashSet<String>,
}

#[derive(Debug)]
pub struct JoinedSnapshot {
    pub peer_id: String,
    pub members: Vec<MemberInfo>,
    pub active_sharers: Vec<String>,
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
        password: &str,
        sender: UnboundedSender<ServerMessage>,
    ) -> (String, JoinedSnapshot) {
        let room_code = generate_room_code();
        let peer_id = Uuid::new_v4().to_string();
        let password_hash = hash_password(password);

        let mut members = HashMap::new();
        members.insert(peer_id.clone(), Member { nick: nick.clone(), sender });

        let mut rooms = self.rooms.lock().unwrap();
        rooms.insert(room_code.clone(), Room { password_hash, members, sharers: HashSet::new() });

        let snapshot = JoinedSnapshot {
            peer_id: peer_id.clone(),
            members: vec![MemberInfo { peer_id, nick }],
            active_sharers: vec![],
        };
        (room_code, snapshot)
    }

    pub fn join_room(
        &self,
        room_code: &str,
        nick: String,
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
            let _ = member
                .sender
                .send(ServerMessage::PeerJoined { peer_id: peer_id.clone(), nick: nick.clone() });
        }

        room.members.insert(peer_id.clone(), Member { nick: nick.clone(), sender });

        let members: Vec<MemberInfo> = room
            .members
            .iter()
            .map(|(id, m)| MemberInfo { peer_id: id.clone(), nick: m.nick.clone() })
            .collect();
        let active_sharers: Vec<String> = room.sharers.iter().cloned().collect();

        Ok(JoinedSnapshot { peer_id, members, active_sharers })
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

        let (room_code, snapshot) = registry.create_room("Ana".to_string(), "senha123", tx);

        assert_eq!(room_code.len(), 8);
        assert_eq!(snapshot.members, vec![MemberInfo { peer_id: snapshot.peer_id.clone(), nick: "Ana".to_string() }]);
        assert!(snapshot.active_sharers.is_empty());
    }

    #[tokio::test]
    async fn join_room_not_found_returns_error() {
        let registry = Registry::new();
        let (tx, _rx) = unbounded_channel();

        let result = registry.join_room("NOPE0000", "Bia".to_string(), "senha123", tx);
        assert_eq!(result.unwrap_err(), JoinError::NotFound);
    }

    #[tokio::test]
    async fn join_room_with_wrong_password_returns_error() {
        let registry = Registry::new();
        let (host_tx, _host_rx) = unbounded_channel();
        let (viewer_tx, _viewer_rx) = unbounded_channel();

        let (room_code, _snapshot) = registry.create_room("Ana".to_string(), "senha123", host_tx);
        let result = registry.join_room(&room_code, "Bia".to_string(), "senha-errada", viewer_tx);

        assert_eq!(result.unwrap_err(), JoinError::WrongPassword);
    }

    #[tokio::test]
    async fn join_room_success_notifies_existing_members_and_includes_them_in_snapshot() {
        let registry = Registry::new();
        let (host_tx, mut host_rx) = unbounded_channel();
        let (viewer_tx, _viewer_rx) = unbounded_channel();

        let (room_code, creator_snapshot) = registry.create_room("Ana".to_string(), "senha123", host_tx);
        let snapshot = registry.join_room(&room_code, "Bia".to_string(), "senha123", viewer_tx).unwrap();

        assert_eq!(snapshot.members.len(), 2);
        assert!(snapshot.members.iter().any(|m| m.peer_id == creator_snapshot.peer_id && m.nick == "Ana"));
        assert!(snapshot.members.iter().any(|m| m.peer_id == snapshot.peer_id && m.nick == "Bia"));

        let notification = host_rx.recv().await.unwrap();
        assert_eq!(notification, ServerMessage::PeerJoined { peer_id: snapshot.peer_id.clone(), nick: "Bia".to_string() });
    }

    #[tokio::test]
    async fn join_room_full_returns_error() {
        let registry = Registry::new();
        let (host_tx, _host_rx) = unbounded_channel();
        let (room_code, _snapshot) = registry.create_room("Membro0".to_string(), "senha123", host_tx);

        for i in 1..MAX_MEMBERS {
            let (tx, _rx) = unbounded_channel();
            registry
                .join_room(&room_code, format!("Membro{i}"), "senha123", tx)
                .expect("deveria caber até MAX_MEMBERS");
        }

        let (extra_tx, _extra_rx) = unbounded_channel();
        let result = registry.join_room(&room_code, "MembroExtra".to_string(), "senha123", extra_tx);
        assert_eq!(result.unwrap_err(), JoinError::Full);
    }

    #[tokio::test]
    async fn start_share_notifies_others_but_not_self() {
        let registry = Registry::new();
        let (host_tx, mut host_rx) = unbounded_channel();
        let (viewer_tx, mut viewer_rx) = unbounded_channel();

        let (room_code, creator_snapshot) = registry.create_room("Ana".to_string(), "senha123", host_tx);
        registry.join_room(&room_code, "Bia".to_string(), "senha123", viewer_tx).unwrap();
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

        let (room_code, creator_snapshot) = registry.create_room("Ana".to_string(), "senha123", host_tx);
        registry.join_room(&room_code, "Bia".to_string(), "senha123", viewer_tx).unwrap();
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

        let (room_code, creator_snapshot) = registry.create_room("Ana".to_string(), "senha123", host_tx);
        registry.join_room(&room_code, "Bia".to_string(), "senha123", viewer_tx).unwrap();

        registry.leave_room(&room_code, &creator_snapshot.peer_id);

        let notification = viewer_rx.recv().await.unwrap();
        assert_eq!(notification, ServerMessage::PeerLeft { peer_id: creator_snapshot.peer_id });

        // A sala continua existindo — entrar de novo com a senha certa funciona.
        let (another_tx, _another_rx) = unbounded_channel();
        assert!(registry.join_room(&room_code, "Caio".to_string(), "senha123", another_tx).is_ok());
    }

    #[tokio::test]
    async fn leave_room_removes_room_when_last_member_leaves() {
        let registry = Registry::new();
        let (host_tx, _host_rx) = unbounded_channel();
        let (room_code, creator_snapshot) = registry.create_room("Ana".to_string(), "senha123", host_tx);

        registry.leave_room(&room_code, &creator_snapshot.peer_id);

        let (tx, _rx) = unbounded_channel();
        let result = registry.join_room(&room_code, "Bia".to_string(), "senha123", tx);
        assert_eq!(result.unwrap_err(), JoinError::NotFound);
    }

    #[tokio::test]
    async fn leave_room_while_sharing_also_sends_peer_stopped_sharing() {
        let registry = Registry::new();
        let (host_tx, _host_rx) = unbounded_channel();
        let (viewer_tx, mut viewer_rx) = unbounded_channel();

        let (room_code, creator_snapshot) = registry.create_room("Ana".to_string(), "senha123", host_tx);
        registry.join_room(&room_code, "Bia".to_string(), "senha123", viewer_tx).unwrap();
        registry.start_share(&room_code, &creator_snapshot.peer_id);
        viewer_rx.recv().await.unwrap(); // drena o PeerStartedSharing

        registry.leave_room(&room_code, &creator_snapshot.peer_id);

        let left = viewer_rx.recv().await.unwrap();
        assert_eq!(left, ServerMessage::PeerLeft { peer_id: creator_snapshot.peer_id.clone() });
        let stopped = viewer_rx.recv().await.unwrap();
        assert_eq!(stopped, ServerMessage::PeerStoppedSharing { peer_id: creator_snapshot.peer_id });
    }
}
```

> Nota: diferente das tasks TDD "puras" (escreva teste vazio → veja falhar → implemente), aqui teste e implementação vêm no mesmo passo porque o arquivo inteiro está sendo reescrito de uma vez (o `Registry` do v1 não compila mais depois da Task 2 mudar o protocolo). O Step 2 abaixo é o que garante que você não pulou a verificação.

- [ ] **Step 2: Rodar os testes e confirmar que passam**

Run: `cargo test --lib --features ssr signaling::registry`
Expected: PASS (9 testes).

- [ ] **Step 3: Commit**

```bash
git add src/signaling/registry.rs
git commit -m "feat: rewrite room registry for authenticated multi-sharer rooms"
```

---

## Task 4: Endpoint WebSocket `/ws` v2

**Files:**
- Modify: `src/signaling/ws.rs`
- Modify: `tests/signaling_ws.rs` (substituir o teste de integração do v1)

**Interfaces:**
- Consumes: `crate::signaling::registry::{Registry, JoinError}` (Task 3), `crate::signaling::protocol::{ClientMessage, ServerMessage}` (Task 2).
- Produces: `crate::signaling::ws::ws_handler` — mesma assinatura de antes, agora roteando as mensagens novas.

- [ ] **Step 1: Escrever o teste de integração que falha (substituindo o do v1)**

Substitua todo o conteúdo de `tests/signaling_ws.rs` por:

```rust
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
    send_json(&mut creator_ws, &ClientMessage::CreateRoom { nick: "Ana".to_string(), password: "senha123".to_string() }).await;

    let room = match recv_json(&mut creator_ws).await {
        ServerMessage::Joined { room, members, .. } => {
            assert_eq!(members.len(), 1);
            room
        }
        other => panic!("esperava Joined, recebeu {other:?}"),
    };

    let (mut viewer_ws, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
    send_json(&mut viewer_ws, &ClientMessage::JoinRoom { room: room.clone(), nick: "Bia".to_string(), password: "senha-errada".to_string() }).await;
    assert_eq!(recv_json(&mut viewer_ws).await, ServerMessage::AuthFailed);

    send_json(&mut viewer_ws, &ClientMessage::JoinRoom { room: room.clone(), nick: "Bia".to_string(), password: "senha123".to_string() }).await;
    let viewer_id = match recv_json(&mut viewer_ws).await {
        ServerMessage::Joined { peer_id, members, .. } => {
            assert_eq!(members.len(), 2);
            peer_id
        }
        other => panic!("esperava Joined, recebeu {other:?}"),
    };

    assert_eq!(recv_json(&mut creator_ws).await, ServerMessage::PeerJoined { peer_id: viewer_id, nick: "Bia".to_string() });
}

#[tokio::test]
async fn start_share_broadcasts_and_offer_is_relayed() {
    let url = spawn_test_server().await;

    let (mut sharer_ws, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
    send_json(&mut sharer_ws, &ClientMessage::CreateRoom { nick: "Ana".to_string(), password: "senha123".to_string() }).await;
    let (room, sharer_id) = match recv_json(&mut sharer_ws).await {
        ServerMessage::Joined { room, peer_id, .. } => (room, peer_id),
        other => panic!("esperava Joined, recebeu {other:?}"),
    };

    let (mut viewer_ws, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
    send_json(&mut viewer_ws, &ClientMessage::JoinRoom { room: room.clone(), nick: "Bia".to_string(), password: "senha123".to_string() }).await;
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
async fn room_not_found_for_unknown_code() {
    let url = spawn_test_server().await;
    let (mut ws, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
    send_json(&mut ws, &ClientMessage::JoinRoom { room: "NOPE0000".to_string(), nick: "Bia".to_string(), password: "x".to_string() }).await;
    assert_eq!(recv_json(&mut ws).await, ServerMessage::RoomNotFound);
}
```

- [ ] **Step 2: Rodar os testes e confirmar que falham**

Run: `cargo test --features ssr --test signaling_ws`
Expected: FAIL — `ws_handler` ainda usa o protocolo antigo (não compila com os novos tipos de `ClientMessage`/`ServerMessage`).

- [ ] **Step 3: Implementar o handler**

Substitua todo o conteúdo de `src/signaling/ws.rs` por:

```rust
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::IntoResponse;
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;

use super::protocol::{ClientMessage, ServerMessage};
use super::registry::{JoinError, Registry};

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
            ClientMessage::CreateRoom { nick, password } => {
                let (code, snapshot) = registry.create_room(nick, &password, tx.clone());
                let _ = tx.send(ServerMessage::Joined {
                    peer_id: snapshot.peer_id.clone(),
                    room: code.clone(),
                    members: snapshot.members,
                    active_sharers: snapshot.active_sharers,
                });
                room_code = Some(code);
                peer_id = Some(snapshot.peer_id);
            }
            ClientMessage::JoinRoom { room, nick, password } => {
                match registry.join_room(&room, nick, &password, tx.clone()) {
                    Ok(snapshot) => {
                        let _ = tx.send(ServerMessage::Joined {
                            peer_id: snapshot.peer_id.clone(),
                            room: room.clone(),
                            members: snapshot.members,
                            active_sharers: snapshot.active_sharers,
                        });
                        peer_id = Some(snapshot.peer_id);
                        room_code = Some(room);
                    }
                    Err(JoinError::NotFound) => {
                        let _ = tx.send(ServerMessage::RoomNotFound);
                    }
                    Err(JoinError::WrongPassword) => {
                        let _ = tx.send(ServerMessage::AuthFailed);
                    }
                    Err(JoinError::Full) => {
                        let _ = tx.send(ServerMessage::RoomFull);
                    }
                }
            }
            ClientMessage::StartShare => {
                if let (Some(room), Some(id)) = (&room_code, &peer_id) {
                    registry.start_share(room, id);
                }
            }
            ClientMessage::StopShare => {
                if let (Some(room), Some(id)) = (&room_code, &peer_id) {
                    registry.stop_share(room, id);
                }
            }
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
            ClientMessage::IceCandidate { to, stream_owner, candidate, sdp_mid, sdp_m_line_index } => {
                if let (Some(room), Some(from)) = (&room_code, &peer_id) {
                    registry.relay(
                        room,
                        &to,
                        ServerMessage::IceCandidate { from: from.clone(), stream_owner, candidate, sdp_mid, sdp_m_line_index },
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
```

- [ ] **Step 4: Rodar os testes e confirmar que passam**

Run: `cargo test --features ssr --test signaling_ws`
Expected: PASS (3 testes).

- [ ] **Step 5: Commit**

```bash
git add src/signaling/ws.rs tests/signaling_ws.rs
git commit -m "feat: wire authenticated multi-sharer protocol to the /ws endpoint"
```

---

## Task 5: Estado do cliente — nick em `localStorage` e handoff de autenticação

**Files:**
- Create: `src/client/storage.rs`
- Create: `src/client/session.rs`
- Modify: `src/client/mod.rs`
- Modify: `src/client/socket.rs` (adicionar `WsClient::set_on_message`)
- Modify: `Cargo.toml` (adicionar `"Storage"` às features do `web-sys`)

**Interfaces:**
- Produces: `crate::client::storage::{load_nick() -> Option<String>, save_nick(nick: &str)}` — usadas pelas Tasks 6, 7, 8. `crate::client::session::{PendingSession, stash(session: PendingSession), take(room: &str) -> Option<PendingSession>}` — usadas pelas Tasks 6 e 7 para entregar uma conexão WebSocket já autenticada da Home pra Room sem reabri-la. `crate::client::socket::WsClient::set_on_message` — troca o handler de mensagens de uma conexão já aberta, usado pela Task 7 ao assumir a conexão da Task 6.

> **Por que não um contexto do Leptos:** a ideia óbvia seria guardar `{room, nick, password}` num `RwSignal` fornecido via `provide_context` em `App` (Task 6 manda, Task 7 lê, Task 7 reabre a conexão do zero com `JoinRoom`). Isso *parecia* funcionar mas tem um bug real: fechar a conexão da Home assim que a sala é criada esvazia a sala (ela fica com 0 membros por um instante) e o servidor a remove antes da Room conseguir reabrir com `JoinRoom` — confirmado em teste manual no navegador (a Room mostrava "Sala não encontrada ou já foi encerrada" logo após criar a sala). A correção é não fechar a conexão: a Home deixa a mesma `WsClient` já autenticada pronta pra Room assumir. Isso não pode viajar por `provide_context`/`use_context`, porque `WsClient` só existe sob a feature `hydrate` e o componente `App` (que registraria o contexto) também é compilado sob `ssr` — um campo desse tipo no contexto quebraria a build do servidor. Por isso o handoff usa um `thread_local!` em `client/session.rs`, que só existe no binário WASM.

- [ ] **Step 1: Adicionar `Storage` às features do `web-sys`**

Em `Cargo.toml`, no bloco `web-sys = { ..., features = [...] }`, adicione `"Storage"` à lista (ex.: logo após `"Location"`).

- [ ] **Step 2: Implementar o helper de `localStorage`**

`src/client/storage.rs`:

```rust
const NICK_KEY: &str = "screen_share_nick";

#[cfg(not(feature = "hydrate"))]
pub fn load_nick() -> Option<String> {
    None
}

#[cfg(feature = "hydrate")]
pub fn load_nick() -> Option<String> {
    let window = web_sys::window()?;
    let storage = window.local_storage().ok()??;
    storage.get_item(NICK_KEY).ok()?
}

#[cfg(not(feature = "hydrate"))]
pub fn save_nick(_nick: &str) {}

#[cfg(feature = "hydrate")]
pub fn save_nick(nick: &str) {
    if let Some(window) = web_sys::window() {
        if let Ok(Some(storage)) = window.local_storage() {
            let _ = storage.set_item(NICK_KEY, nick);
        }
    }
}
```

`src/client/mod.rs` — adicione:

```rust
pub mod storage;
```

- [ ] **Step 3: Implementar o handoff de sessão (`client/session.rs`) e `WsClient::set_on_message`**

`src/client/session.rs`:

```rust
use std::cell::RefCell;

use crate::client::socket::WsClient;
use crate::signaling::protocol::MemberInfo;

/// Uma conexão já autenticada (via `CreateRoom`) que a `HomePage` deixa
/// pronta pra `RoomPage` assumir, sem reabrir o WebSocket nem repetir a
/// senha. Guardado num `thread_local` — não passa pelo sistema de contexto
/// do Leptos porque `WsClient` só existe sob a feature `hydrate`, e o
/// componente `App` (que registraria o contexto) também é compilado sob
/// `ssr`.
pub struct PendingSession {
    pub room: String,
    pub ws: WsClient,
    pub peer_id: String,
    pub members: Vec<MemberInfo>,
    pub active_sharers: Vec<String>,
}

thread_local! {
    static PENDING: RefCell<Option<PendingSession>> = const { RefCell::new(None) };
}

pub fn stash(session: PendingSession) {
    PENDING.with(|cell| *cell.borrow_mut() = Some(session));
}

/// Retira a sessão pendente somente se ela for para a sala pedida — evita
/// que uma sala criada e depois abandonada (ex.: o usuário voltou pra `/` e
/// criou outra) vaze pra uma `RoomPage` diferente.
pub fn take(room: &str) -> Option<PendingSession> {
    PENDING.with(|cell| {
        let mut slot = cell.borrow_mut();
        if slot.as_ref().map(|s| s.room.as_str()) == Some(room) {
            slot.take()
        } else {
            None
        }
    })
}
```

`src/client/mod.rs` — adicione também:

```rust
pub mod session;
```

Em `src/client/socket.rs`, dentro do `impl WsClient` (dentro `#[cfg(feature = "hydrate")]`, já que o tipo inteiro só existe ali), adicione:

```rust
/// Substitui o handler de mensagens de uma conexão já aberta. Usado
/// quando a `RoomPage` assume uma conexão que a `HomePage` deixou
/// autenticada (ver `client::session`) — a conexão continua sendo a
/// mesma (mesmo `peer_id` no servidor), só o código que reage às
/// mensagens seguintes muda.
pub fn set_on_message(&mut self, on_message: impl Fn(ServerMessage) + 'static) {
    let on_message_cb = Closure::<dyn FnMut(MessageEvent)>::new(move |event: MessageEvent| {
        if let Some(text) = event.data().as_string() {
            if let Ok(msg) = serde_json::from_str::<ServerMessage>(&text) {
                on_message(msg);
            }
        }
    });
    self.socket.set_onmessage(Some(on_message_cb.as_ref().unchecked_ref()));
    self._on_message = on_message_cb;
}
```

Isso exige que o campo `socket` de `WsClient` seja acessível dentro do próprio `impl` (já é, por estar no mesmo módulo) e reaproveita a mesma lógica de parsing de `WsClient::connect`.

- [ ] **Step 4: Verificar manualmente no navegador**

Run: `cargo leptos watch`, abra `http://127.0.0.1:3000/`, abra o console do navegador e rode:

```js
localStorage.setItem("screen_share_nick", "teste-manual");
```

Recarregue a página — não deve haver nenhum erro no console (a `HomePage` ainda não lê isso, será usado na Task 6; este passo só confirma que `Storage` foi habilitado sem quebrar a build WASM).

- [ ] **Step 5: Commit**

```bash
git add src/client/storage.rs src/client/session.rs src/client/mod.rs src/client/socket.rs Cargo.toml
git commit -m "feat: add nick localStorage persistence and session handoff"
```

---

## Task 6: Página inicial — formulário de criar sala

**Files:**
- Modify: `src/pages/home.rs` (substituir por completo o conteúdo do v1)

**Interfaces:**
- Consumes: `crate::client::socket::WsClient`, `crate::client::storage::save_nick`, `crate::client::session::{self, PendingSession}` (Task 5), `crate::signaling::protocol::{ClientMessage, ServerMessage}` (Task 2).

- [ ] **Step 1: Substituir `src/pages/home.rs`**

```rust
use leptos::prelude::*;

use crate::pages::status::status_meta;

#[component]
pub fn HomePage() -> impl IntoView {
    let (nick, set_nick) = signal(initial_nick());
    let (password, set_password) = signal(String::new());
    let (status, set_status) = signal("Pronto para criar uma sala.".to_string());
    let (submitting, set_submitting) = signal(false);

    let create_room = create_room_handler(nick, password, set_status, set_submitting);

    view! {
        <div class="panel">
            <h1>"Criar sala"</h1>
            <p class="subtext">"Escolha um nick e uma senha. Compartilhe o link e a senha com quem você quiser na sala."</p>

            <form on:submit=create_room>
                <label class="field">
                    <span class="field__label">"Nick"</span>
                    <input
                        class="field__input"
                        type="text"
                        required
                        prop:value=nick
                        on:input:target=move |ev| set_nick.set(ev.target().value())
                    />
                </label>
                <label class="field">
                    <span class="field__label">"Senha da sala"</span>
                    <input
                        class="field__input"
                        type="password"
                        required
                        prop:value=password
                        on:input:target=move |ev| set_password.set(ev.target().value())
                    />
                </label>
                <button class="btn btn--primary" type="submit" disabled=submitting>
                    {move || if submitting.get() { "Criando..." } else { "Criar sala" }}
                </button>
            </form>

            <p class="status-text" class:status-text--error=move || status_meta(&status.get()).0 == "error">
                {status}
            </p>
        </div>
    }
}

#[cfg(not(feature = "hydrate"))]
fn initial_nick() -> String {
    String::new()
}

#[cfg(feature = "hydrate")]
fn initial_nick() -> String {
    crate::client::storage::load_nick().unwrap_or_default()
}

#[cfg(not(feature = "hydrate"))]
fn create_room_handler(
    _nick: ReadSignal<String>,
    _password: ReadSignal<String>,
    _set_status: WriteSignal<String>,
    _set_submitting: WriteSignal<bool>,
) -> impl Fn(leptos::ev::SubmitEvent) + 'static {
    move |ev: leptos::ev::SubmitEvent| ev.prevent_default()
}

#[cfg(feature = "hydrate")]
fn create_room_handler(
    nick: ReadSignal<String>,
    password: ReadSignal<String>,
    set_status: WriteSignal<String>,
    set_submitting: WriteSignal<bool>,
) -> impl Fn(leptos::ev::SubmitEvent) + 'static {
    use std::cell::RefCell;
    use std::rc::Rc;

    use leptos_router::hooks::use_navigate;

    use crate::client::session::{self, PendingSession};
    use crate::client::socket::WsClient;
    use crate::client::storage::save_nick;
    use crate::signaling::protocol::{ClientMessage, ServerMessage};

    move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();

        let nick_value = nick.get_untracked().trim().to_string();
        let password_value = password.get_untracked();
        if nick_value.is_empty() || password_value.is_empty() {
            set_status.set("Preencha nick e senha.".to_string());
            return;
        }

        set_submitting.set(true);
        set_status.set("Criando sala...".to_string());

        let ws_slot: Rc<RefCell<Option<WsClient>>> = Rc::new(RefCell::new(None));
        let navigate = use_navigate();

        let on_message = {
            let ws_slot = ws_slot.clone();
            let nick_value = nick_value.clone();
            move |msg: ServerMessage| {
                if let ServerMessage::Joined { peer_id, room, members, active_sharers } = msg {
                    // Não fecha nem reabre a conexão: a RoomPage assume esta
                    // mesma conexão já autenticada (ver `client::session`).
                    // Fechá-la aqui esvaziaria a sala (ela teria 0 membros
                    // por um instante) e o servidor a removeria antes da
                    // RoomPage conseguir entrar.
                    save_nick(&nick_value);
                    if let Some(ws) = ws_slot.borrow_mut().take() {
                        session::stash(PendingSession { room: room.clone(), ws, peer_id, members, active_sharers });
                    }
                    navigate(&format!("/r/{room}"), Default::default());
                }
            }
        };

        match WsClient::connect("/ws", on_message) {
            Ok(ws) => {
                ws.on_open({
                    let ws_slot = ws_slot.clone();
                    let nick_for_open = nick_value.clone();
                    let password_for_open = password_value.clone();
                    move || {
                        if let Some(ws) = ws_slot.borrow().as_ref() {
                            ws.send(&ClientMessage::CreateRoom {
                                nick: nick_for_open.clone(),
                                password: password_for_open.clone(),
                            });
                        }
                    }
                });
                *ws_slot.borrow_mut() = Some(ws);
            }
            Err(_) => {
                set_submitting.set(false);
                set_status.set("Não foi possível conectar ao servidor.".to_string());
            }
        }
    }
}
```

- [ ] **Step 2: Verificar manualmente no navegador**

Run: `cargo leptos watch`, abra `http://127.0.0.1:3000/`.
Expected: formulário com nick + senha; ao enviar, navega para `/r/<código>` e a Room já entra autenticada direto (ver Task 7 — a conexão é assumida, não reaberta). Nenhum erro no console.

- [ ] **Step 3: Commit**

```bash
git add src/pages/home.rs
git commit -m "feat: replace direct-share home page with create-room form"
```

---

## Task 7: Página de sala — portão de autenticação e lista de membros

**Files:**
- Modify: `src/pages/room.rs` (substituir por completo o conteúdo do v1)

**Interfaces:**
- Consumes: `crate::client::socket::WsClient`, `crate::client::storage::{load_nick, save_nick}`, `crate::client::session` (Task 5), `crate::signaling::protocol::{ClientMessage, ServerMessage, MemberInfo}` (Task 2).
- Produces: `crate::pages::room::RoomMember { peer_id: String, nick: String, sharing: bool }` (`Clone + PartialEq`) — usada pela Task 8. `RoomConnection` (struct interna, só `hydrate`) — a Task 8 estende esta mesma struct em vez de criar outra.

- [ ] **Step 1: Substituir `src/pages/room.rs`**

```rust
use leptos::prelude::*;
use leptos_router::hooks::use_params_map;

use crate::pages::status::status_meta;

#[derive(Clone, PartialEq)]
pub struct RoomMember {
    pub peer_id: String,
    pub nick: String,
    pub sharing: bool,
}

// A Task 8 estende esta struct (adiciona `outgoing`, `incoming`,
// `local_stream`) em vez de criar uma nova — ela já existe aqui porque
// `adopt_pending_session`, abaixo, precisa de um lugar pra guardar a
// `WsClient` assumida da Home.
#[cfg(feature = "hydrate")]
#[derive(Clone)]
struct RoomConnection {
    ws: std::rc::Rc<std::cell::RefCell<Option<crate::client::socket::WsClient>>>,
}

#[cfg(feature = "hydrate")]
impl RoomConnection {
    fn new() -> Self {
        Self { ws: Default::default() }
    }
}

#[cfg(not(feature = "hydrate"))]
#[derive(Clone)]
struct RoomConnection;

#[cfg(not(feature = "hydrate"))]
impl RoomConnection {
    fn new() -> Self {
        Self
    }
}

#[component]
pub fn RoomPage() -> impl IntoView {
    let params = use_params_map();
    let code = move || params.read().get("code").unwrap_or_default();
    let initial_code = params.read_untracked().get("code").unwrap_or_default();

    let (nick, set_nick) = signal(initial_nick());
    let (password, set_password) = signal(String::new());
    let (status, set_status) = signal("Informe o nick e a senha da sala.".to_string());
    let (authenticated, set_authenticated) = signal(false);
    let (members, set_members) = signal(Vec::<RoomMember>::new());
    let (_my_peer_id, set_my_peer_id) = signal(None::<String>);

    let conn = RoomConnection::new();

    let join_room = setup_room_connection(
        initial_code.clone(),
        conn.clone(),
        set_status,
        set_authenticated,
        set_members,
        set_my_peer_id,
    );

    // Se viemos da criação da sala na home, a conexão já está autenticada
    // (ver `client::session`) — assume ela em vez de pedir nick/senha de
    // novo. Chamada direta (não via Effect): `initial_code` já está
    // disponível de forma síncrona na montagem do componente.
    adopt_pending_session(initial_code, conn, set_status, set_authenticated, set_members, set_my_peer_id);

    let manual_join = {
        let join_room = join_room.clone();
        move |ev: leptos::ev::SubmitEvent| {
            ev.prevent_default();
            let nick_value = nick.get_untracked().trim().to_string();
            let password_value = password.get_untracked();
            if nick_value.is_empty() || password_value.is_empty() {
                set_status.set("Preencha nick e senha.".to_string());
                return;
            }
            join_room(nick_value, password_value);
        }
    };

    let lamp_class = move || {
        let (variant, _) = status_meta(&status.get());
        format!("lamp lamp--{variant}")
    };

    view! {
        // As duas seções ficam sempre montadas e alternam por CSS
        // (class:hidden), não por montagem/desmontagem condicional
        // (`<Show>`): o Leptos 0.8 exige que qualquer closure de filho
        // dinâmico (o que `<Show>` usa para seus filhos e para `fallback`)
        // seja Send + Sync, mesmo rodando single-threaded no navegador — e o
        // formulário de entrada captura um `Rc<RefCell<WsClient>>` (via
        // `manual_join` → `join_room`), que não é. Mantendo o formulário
        // como filho estático (avaliado uma vez) e só alternando a classe
        // evita esse requisito, no mesmo espírito do padrão "estado por
        // classificação, não por montagem" que o resto do app já usa (ver
        // `CLAUDE.md`, seção "Status-driven UI").
        <div class="panel" class:hidden=move || authenticated.get()>
            <h1>"Entrar na sala"</h1>
            <p class="status-row__meta">{code}</p>
            <form on:submit=manual_join.clone()>
                <label class="field">
                    <span class="field__label">"Nick"</span>
                    <input class="field__input" type="text" required prop:value=nick
                        on:input:target=move |ev| set_nick.set(ev.target().value())/>
                </label>
                <label class="field">
                    <span class="field__label">"Senha da sala"</span>
                    <input class="field__input" type="password" required prop:value=password
                        on:input:target=move |ev| set_password.set(ev.target().value())/>
                </label>
                <button class="btn btn--primary" type="submit">"Entrar"</button>
            </form>
            <p class="status-text" class:status-text--error=move || status_meta(&status.get()).0 == "error">
                {status}
            </p>
        </div>
        <div class="room-page" class:hidden=move || !authenticated.get()>
            <div class="stage-header">
                <span class=lamp_class></span>
                <span class="status-row__meta">{code}</span>
            </div>
            <div class="grid">
                <For
                    each=move || members.get()
                    key=|m| m.peer_id.clone()
                    let(member)
                >
                    <div class="tile">
                        <div class="tile__label">
                            {member.nick.clone()}
                            {move || if member.sharing { " (compartilhando)" } else { "" }}
                        </div>
                    </div>
                </For>
            </div>
        </div>
    }
}

#[cfg(not(feature = "hydrate"))]
fn initial_nick() -> String {
    String::new()
}

#[cfg(feature = "hydrate")]
fn initial_nick() -> String {
    crate::client::storage::load_nick().unwrap_or_default()
}

#[cfg(feature = "hydrate")]
fn apply_joined_snapshot(
    peer_id: String,
    joined_members: Vec<crate::signaling::protocol::MemberInfo>,
    active_sharers: Vec<String>,
    set_my_peer_id: WriteSignal<Option<String>>,
    set_members: WriteSignal<Vec<RoomMember>>,
    set_authenticated: WriteSignal<bool>,
    set_status: WriteSignal<String>,
) {
    use std::collections::HashSet;

    let sharer_set: HashSet<String> = active_sharers.into_iter().collect();
    let members: Vec<RoomMember> = joined_members
        .into_iter()
        .map(|m| RoomMember { sharing: sharer_set.contains(&m.peer_id), peer_id: m.peer_id, nick: m.nick })
        .collect();
    set_my_peer_id.set(Some(peer_id));
    set_members.set(members);
    set_authenticated.set(true);
    set_status.set("Conectado.".to_string());
}

#[cfg(feature = "hydrate")]
fn build_message_handler(
    set_status: WriteSignal<String>,
    set_authenticated: WriteSignal<bool>,
    set_members: WriteSignal<Vec<RoomMember>>,
    set_my_peer_id: WriteSignal<Option<String>>,
) -> impl Fn(crate::signaling::protocol::ServerMessage) + 'static {
    use crate::signaling::protocol::ServerMessage;

    move |msg: ServerMessage| match msg {
        ServerMessage::Joined { peer_id, members: joined_members, active_sharers, .. } => {
            apply_joined_snapshot(peer_id, joined_members, active_sharers, set_my_peer_id, set_members, set_authenticated, set_status);
        }
        ServerMessage::AuthFailed => set_status.set("Senha incorreta.".to_string()),
        ServerMessage::RoomNotFound => set_status.set("Sala não encontrada ou já foi encerrada.".to_string()),
        ServerMessage::RoomFull => set_status.set("Essa sala já está cheia (máximo de 8 pessoas).".to_string()),
        ServerMessage::PeerJoined { peer_id, nick } => {
            set_members.update(|members| members.push(RoomMember { peer_id, nick, sharing: false }));
        }
        ServerMessage::PeerLeft { peer_id } => {
            set_members.update(|members| members.retain(|m| m.peer_id != peer_id));
        }
        ServerMessage::PeerStartedSharing { peer_id } => {
            set_members.update(|members| {
                if let Some(m) = members.iter_mut().find(|m| m.peer_id == peer_id) {
                    m.sharing = true;
                }
            });
        }
        ServerMessage::PeerStoppedSharing { peer_id } => {
            set_members.update(|members| {
                if let Some(m) = members.iter_mut().find(|m| m.peer_id == peer_id) {
                    m.sharing = false;
                }
            });
        }
        _ => {}
    }
}

#[cfg(not(feature = "hydrate"))]
fn adopt_pending_session(
    _room_code: String,
    _conn: RoomConnection,
    _set_status: WriteSignal<String>,
    _set_authenticated: WriteSignal<bool>,
    _set_members: WriteSignal<Vec<RoomMember>>,
    _set_my_peer_id: WriteSignal<Option<String>>,
) {
}

#[cfg(feature = "hydrate")]
fn adopt_pending_session(
    room_code: String,
    conn: RoomConnection,
    set_status: WriteSignal<String>,
    set_authenticated: WriteSignal<bool>,
    set_members: WriteSignal<Vec<RoomMember>>,
    set_my_peer_id: WriteSignal<Option<String>>,
) {
    use crate::client::session;

    let Some(mut session) = session::take(&room_code) else { return };

    let on_message = build_message_handler(set_status, set_authenticated, set_members, set_my_peer_id);
    session.ws.set_on_message(on_message);
    session.ws.on_close(move || {
        set_status.set("Conexão perdida. Recarregue a página para tentar de novo.".to_string());
    });

    // Aplica o snapshot que a Home já recebeu no `Joined` original — não faz
    // um novo round-trip de rede, já temos os dados.
    apply_joined_snapshot(
        session.peer_id,
        session.members,
        session.active_sharers,
        set_my_peer_id,
        set_members,
        set_authenticated,
        set_status,
    );

    *conn.ws.borrow_mut() = Some(session.ws);
}

#[cfg(not(feature = "hydrate"))]
fn setup_room_connection(
    _room_code: String,
    _conn: RoomConnection,
    _set_status: WriteSignal<String>,
    _set_authenticated: WriteSignal<bool>,
    _set_members: WriteSignal<Vec<RoomMember>>,
    _set_my_peer_id: WriteSignal<Option<String>>,
) -> impl Fn(String, String) + Clone + 'static {
    move |_nick: String, _password: String| {}
}

#[cfg(feature = "hydrate")]
fn setup_room_connection(
    room_code: String,
    conn: RoomConnection,
    set_status: WriteSignal<String>,
    set_authenticated: WriteSignal<bool>,
    set_members: WriteSignal<Vec<RoomMember>>,
    set_my_peer_id: WriteSignal<Option<String>>,
) -> impl Fn(String, String) + Clone + 'static {
    use crate::client::socket::WsClient;
    use crate::client::storage::save_nick;
    use crate::signaling::protocol::ClientMessage;

    move |nick: String, password: String| {
        let conn = conn.clone();
        let room_code = room_code.clone();
        set_status.set("Conectando...".to_string());

        let on_message = build_message_handler(set_status, set_authenticated, set_members, set_my_peer_id);

        match WsClient::connect("/ws", on_message) {
            Ok(ws) => {
                ws.on_open({
                    let conn = conn.clone();
                    let room_code = room_code.clone();
                    let nick = nick.clone();
                    let password = password.clone();
                    move || {
                        if let Some(ws) = conn.ws.borrow().as_ref() {
                            ws.send(&ClientMessage::JoinRoom { room: room_code.clone(), nick: nick.clone(), password: password.clone() });
                        }
                    }
                });
                ws.on_close(move || {
                    set_status.set("Conexão perdida. Recarregue a página para tentar de novo.".to_string());
                });
                *conn.ws.borrow_mut() = Some(ws);
                save_nick(&nick);
            }
            Err(_) => set_status.set("Não foi possível conectar ao servidor.".to_string()),
        }
    }
}
```

> Duas conexões são possíveis para chegar autenticado numa sala: `adopt_pending_session` (veio da Home, criou a sala) e `setup_room_connection` (digitou nick/senha na própria Room, seja porque abriu o link direto, seja porque recarregou a página). As duas convergem em `apply_joined_snapshot`/`build_message_handler` pra não duplicar a lógica de aplicar o snapshot de `Joined` e reagir às mensagens seguintes.

- [ ] **Step 2: Verificar manualmente no navegador**

Run: `cargo leptos watch`
1. Abra `http://127.0.0.1:3000/`, crie uma sala com nick "Ana" e senha "teste123" — Expected: navega para `/r/<código>` e entra direto (sem formulário), mostra "Ana" na grade.
2. Numa aba separada (**sem fechar a primeira** — fechar a aba/navegar pra fora dela derruba a conexão e, como a Ana seria a única integrante, o servidor apaga a sala), abra o mesmo link — Expected: pede nick + senha. Digite a senha errada — Expected: "Senha incorreta.". Digite a certa com outro nick (ex. "Bruno") — Expected: entra, e as duas abas mostram os dois nicks na grade.
3. Abra um link com um código inexistente (ex. `/r/ZZZZZZZZ`) — Expected: "Sala não encontrada ou já foi encerrada."

- [ ] **Step 3: Commit**

```bash
git add src/pages/room.rs
git commit -m "feat: add room auth gate and live member/sharer roster"
```

---

## Task 8: Página de sala — compartilhamento múltiplo via WebRTC

**Files:**
- Modify: `src/pages/room.rs` (adicionar estado de conexão compartilhado, o botão de compartilhar/parar, e o roteamento de `Offer`/`Answer`/`IceCandidate`)

**Interfaces:**
- Consumes: `crate::client::webrtc::{capture_display, new_peer_connection, create_offer, create_answer, accept_answer, add_ice_candidate, is_display_media_supported}` (já existem desde o v1, sem alterações), `RoomMember`, `RoomConnection`, `build_message_handler`, `adopt_pending_session`, `setup_room_connection` (Task 7).

> A Task 7 já deixou `RoomConnection` (só com `ws`), `apply_joined_snapshot`, `build_message_handler`, `adopt_pending_session` e `setup_room_connection` prontos — foi o jeito de resolver o handoff de sessão da Home sem contexto do Leptos (ver a nota na Task 5). Esta task **estende** essas peças em vez de recriá-las: os campos de WebRTC entram em `RoomConnection`, e o roteamento de `Offer`/`Answer`/`IceCandidate` entra em `build_message_handler` — que é chamado tanto por quem chega via `adopt_pending_session` (criou a sala) quanto por quem chega via `setup_room_connection` (digitou nick/senha), então as duas vias precisam do roteamento igualmente. Não duplique essa lógica dentro de `setup_room_connection` como um `on_message` à parte — quem adota a sessão pendente (o caso mais comum: todo criador de sala) ficaria sem receber `Offer`/`Answer`/`IceCandidate`.

- [ ] **Step 1: Estender `RoomConnection` com os mapas de conexão WebRTC**

Em `src/pages/room.rs`, na `struct RoomConnection` que a Task 7 já criou (variante `#[cfg(feature = "hydrate")]`), adicione os três campos novos e atualize `RoomConnection::new()` de acordo:

```rust
#[cfg(feature = "hydrate")]
#[derive(Clone)]
struct RoomConnection {
    ws: std::rc::Rc<std::cell::RefCell<Option<crate::client::socket::WsClient>>>,
    outgoing: std::rc::Rc<std::cell::RefCell<std::collections::HashMap<String, web_sys::RtcPeerConnection>>>,
    incoming: std::rc::Rc<std::cell::RefCell<std::collections::HashMap<String, web_sys::RtcPeerConnection>>>,
    local_stream: std::rc::Rc<std::cell::RefCell<Option<web_sys::MediaStream>>>,
}

#[cfg(feature = "hydrate")]
impl RoomConnection {
    fn new() -> Self {
        Self {
            ws: Default::default(),
            outgoing: Default::default(),
            incoming: Default::default(),
            local_stream: Default::default(),
        }
    }
}
```

(A variante `#[cfg(not(feature = "hydrate"))]` — `struct RoomConnection;` — não muda; ela é só um stub pro lado `ssr`.)

- [ ] **Step 2: Estender `build_message_handler` com o roteamento de `Offer`/`Answer`/`IceCandidate` e a limpeza de conexões em `PeerLeft`/`PeerStoppedSharing`**

`build_message_handler` (Task 7) ganha dois parâmetros novos — `conn: RoomConnection` e `connection_errors: RwSignal<std::collections::HashSet<String>>` — logo após `set_my_peer_id`, e o `match` ganha limpeza de conexão em dois braços existentes mais três braços novos:

```rust
#[cfg(feature = "hydrate")]
fn build_message_handler(
    set_status: WriteSignal<String>,
    set_authenticated: WriteSignal<bool>,
    set_members: WriteSignal<Vec<RoomMember>>,
    set_my_peer_id: WriteSignal<Option<String>>,
    conn: RoomConnection,
    connection_errors: RwSignal<std::collections::HashSet<String>>,
) -> impl Fn(crate::signaling::protocol::ServerMessage) + 'static {
    use leptos::task::spawn_local;
    use wasm_bindgen::JsCast;
    use web_sys::{MediaStream, RtcPeerConnectionIceEvent, RtcTrackEvent};

    use crate::client::webrtc::{accept_answer, add_ice_candidate, create_answer, new_peer_connection};
    use crate::signaling::protocol::{ClientMessage, ServerMessage};

    move |msg: ServerMessage| match msg {
        ServerMessage::Joined { peer_id, members: joined_members, active_sharers, .. } => {
            apply_joined_snapshot(peer_id, joined_members, active_sharers, set_my_peer_id, set_members, set_authenticated, set_status);
        }
        ServerMessage::AuthFailed => set_status.set("Senha incorreta.".to_string()),
        ServerMessage::RoomNotFound => set_status.set("Sala não encontrada ou já foi encerrada.".to_string()),
        ServerMessage::RoomFull => set_status.set("Essa sala já está cheia (máximo de 8 pessoas).".to_string()),
        ServerMessage::PeerJoined { peer_id, nick } => {
            set_members.update(|members| members.push(RoomMember { peer_id, nick, sharing: false }));
        }
        ServerMessage::PeerLeft { peer_id } => {
            set_members.update(|members| members.retain(|m| m.peer_id != peer_id));
            conn.outgoing.borrow_mut().remove(&peer_id).map(|pc| pc.close());
            conn.incoming.borrow_mut().remove(&peer_id).map(|pc| pc.close());
        }
        ServerMessage::PeerStartedSharing { peer_id } => {
            set_members.update(|members| {
                if let Some(m) = members.iter_mut().find(|m| m.peer_id == peer_id) {
                    m.sharing = true;
                }
            });
        }
        ServerMessage::PeerStoppedSharing { peer_id } => {
            set_members.update(|members| {
                if let Some(m) = members.iter_mut().find(|m| m.peer_id == peer_id) {
                    m.sharing = false;
                }
            });
            if let Some(pc) = conn.incoming.borrow_mut().remove(&peer_id) {
                pc.close();
            }
        }
        ServerMessage::Offer { from, sdp } => {
            let conn = conn.clone();
            spawn_local(async move {
                let Ok(pc) = new_peer_connection() else { return };
                conn.incoming.borrow_mut().insert(from.clone(), pc.clone());
                connection_errors.update(|errors| { errors.remove(&from); });

                let sharer_id = from.clone();
                let ontrack = wasm_bindgen::prelude::Closure::<dyn FnMut(RtcTrackEvent)>::new(move |event: RtcTrackEvent| {
                    if let Ok(stream) = event.streams().get(0).dyn_into::<MediaStream>() {
                        if let Some(document) = web_sys::window().and_then(|w| w.document()) {
                            if let Some(video_el) = document.get_element_by_id(&format!("video-{sharer_id}")) {
                                let video: web_sys::HtmlVideoElement = video_el.unchecked_into();
                                video.set_src_object(Some(&stream));
                                let _ = video.play();
                            }
                        }
                    }
                });
                pc.set_ontrack(Some(ontrack.as_ref().unchecked_ref()));
                ontrack.forget();

                let target_id = from.clone();
                let conn_for_ice = conn.clone();
                let onicecandidate = wasm_bindgen::prelude::Closure::<dyn FnMut(RtcPeerConnectionIceEvent)>::new(move |event: RtcPeerConnectionIceEvent| {
                    if let Some(candidate) = event.candidate() {
                        if let Some(ws) = conn_for_ice.ws.borrow().as_ref() {
                            ws.send(&ClientMessage::IceCandidate {
                                to: target_id.clone(),
                                stream_owner: target_id.clone(),
                                candidate: candidate.candidate(),
                                sdp_mid: candidate.sdp_mid(),
                                sdp_m_line_index: candidate.sdp_m_line_index(),
                            });
                        }
                    }
                });
                pc.set_onicecandidate(Some(onicecandidate.as_ref().unchecked_ref()));
                onicecandidate.forget();

                // Isola a falha: só o tile desse sharer específico vira
                // erro, o resto da sala continua recebendo vídeo normal.
                let failed_peer_id = from.clone();
                let oniceconnectionstatechange = {
                    let pc_for_state = pc.clone();
                    wasm_bindgen::prelude::Closure::<dyn FnMut()>::new(move || {
                        if pc_for_state.ice_connection_state() == web_sys::RtcIceConnectionState::Failed {
                            connection_errors.update(|errors| { errors.insert(failed_peer_id.clone()); });
                        }
                    })
                };
                pc.set_oniceconnectionstatechange(Some(oniceconnectionstatechange.as_ref().unchecked_ref()));
                oniceconnectionstatechange.forget();

                if let Ok(answer_sdp) = create_answer(&pc, &sdp).await {
                    if let Some(ws) = conn.ws.borrow().as_ref() {
                        ws.send(&ClientMessage::Answer { to: from.clone(), sdp: answer_sdp });
                    }
                }
            });
        }
        ServerMessage::Answer { from, sdp } => {
            if let Some(pc) = conn.outgoing.borrow().get(&from).cloned() {
                spawn_local(async move {
                    let _ = accept_answer(&pc, &sdp).await;
                });
            }
        }
        ServerMessage::IceCandidate { from, stream_owner, candidate, sdp_mid, sdp_m_line_index } => {
            let pc = if stream_owner == from {
                conn.incoming.borrow().get(&from).cloned()
            } else {
                conn.outgoing.borrow().get(&from).cloned()
            };
            if let Some(pc) = pc {
                add_ice_candidate(&pc, &candidate, sdp_mid, sdp_m_line_index);
            }
        }
    }
}
```

> A rota `stream_owner == from` decide se a mensagem é sobre a conexão em que `from` está me enviando a tela dele (`incoming`) ou sobre a conexão em que eu estou enviando a minha tela pra ele (`outgoing`) — ver a explicação completa no protocolo da spec. Não dá pra simplificar pra "sempre olhar os dois mapas" porque um par pode ter as duas conexões abertas ao mesmo tempo (os dois compartilhando um pro outro).

- [ ] **Step 3: Threadar `conn` e `connection_errors` por `adopt_pending_session` e `setup_room_connection`**

Ambas as funções (variantes `hydrate` e stub) ganham um parâmetro `connection_errors: RwSignal<std::collections::HashSet<String>>` (a `conn: RoomConnection` elas já recebem desde a Task 7) e repassam os dois pra `build_message_handler` na chamada que já existe:

```rust
#[cfg(not(feature = "hydrate"))]
fn adopt_pending_session(
    _room_code: String,
    _conn: RoomConnection,
    _set_status: WriteSignal<String>,
    _set_authenticated: WriteSignal<bool>,
    _set_members: WriteSignal<Vec<RoomMember>>,
    _set_my_peer_id: WriteSignal<Option<String>>,
    _connection_errors: RwSignal<std::collections::HashSet<String>>,
) {
}

#[cfg(feature = "hydrate")]
fn adopt_pending_session(
    room_code: String,
    conn: RoomConnection,
    set_status: WriteSignal<String>,
    set_authenticated: WriteSignal<bool>,
    set_members: WriteSignal<Vec<RoomMember>>,
    set_my_peer_id: WriteSignal<Option<String>>,
    connection_errors: RwSignal<std::collections::HashSet<String>>,
) {
    use crate::client::session;

    let Some(mut session) = session::take(&room_code) else { return };

    let on_message = build_message_handler(set_status, set_authenticated, set_members, set_my_peer_id, conn.clone(), connection_errors);
    session.ws.set_on_message(on_message);
    session.ws.on_close(move || {
        set_status.set("Conexão perdida. Recarregue a página para tentar de novo.".to_string());
    });

    apply_joined_snapshot(
        session.peer_id,
        session.members,
        session.active_sharers,
        set_my_peer_id,
        set_members,
        set_authenticated,
        set_status,
    );

    *conn.ws.borrow_mut() = Some(session.ws);
}

#[cfg(not(feature = "hydrate"))]
fn setup_room_connection(
    _room_code: String,
    _conn: RoomConnection,
    _set_status: WriteSignal<String>,
    _set_authenticated: WriteSignal<bool>,
    _set_members: WriteSignal<Vec<RoomMember>>,
    _set_my_peer_id: WriteSignal<Option<String>>,
    _connection_errors: RwSignal<std::collections::HashSet<String>>,
) -> impl Fn(String, String) + Clone + 'static {
    move |_nick: String, _password: String| {}
}

#[cfg(feature = "hydrate")]
fn setup_room_connection(
    room_code: String,
    conn: RoomConnection,
    set_status: WriteSignal<String>,
    set_authenticated: WriteSignal<bool>,
    set_members: WriteSignal<Vec<RoomMember>>,
    set_my_peer_id: WriteSignal<Option<String>>,
    connection_errors: RwSignal<std::collections::HashSet<String>>,
) -> impl Fn(String, String) + Clone + 'static {
    use crate::client::socket::WsClient;
    use crate::client::storage::save_nick;
    use crate::signaling::protocol::ClientMessage;

    move |nick: String, password: String| {
        let conn = conn.clone();
        let room_code = room_code.clone();
        set_status.set("Conectando...".to_string());

        let on_message = build_message_handler(set_status, set_authenticated, set_members, set_my_peer_id, conn.clone(), connection_errors);

        match WsClient::connect("/ws", on_message) {
            Ok(ws) => {
                ws.on_open({
                    let conn = conn.clone();
                    let room_code = room_code.clone();
                    let nick = nick.clone();
                    let password = password.clone();
                    move || {
                        if let Some(ws) = conn.ws.borrow().as_ref() {
                            ws.send(&ClientMessage::JoinRoom { room: room_code.clone(), nick: nick.clone(), password: password.clone() });
                        }
                    }
                });
                ws.on_close(move || {
                    set_status.set("Conexão perdida. Recarregue a página para tentar de novo.".to_string());
                });
                *conn.ws.borrow_mut() = Some(ws);
                save_nick(&nick);
            }
            Err(_) => set_status.set("Não foi possível conectar ao servidor.".to_string()),
        }
    }
}
```

- [ ] **Step 4: Adicionar o botão de compartilhar/parar e a lógica de oferta**

No final de `src/pages/room.rs`, adicione:

```rust
#[cfg(not(feature = "hydrate"))]
fn share_supported() -> bool {
    true
}

#[cfg(feature = "hydrate")]
fn share_supported() -> bool {
    crate::client::webrtc::is_display_media_supported()
}

#[cfg(not(feature = "hydrate"))]
fn share_toggle_handler(
    _conn: RoomConnection,
    _members: ReadSignal<Vec<RoomMember>>,
    _my_peer_id: ReadSignal<Option<String>>,
    _is_sharing: ReadSignal<bool>,
    _set_is_sharing: WriteSignal<bool>,
    _set_status: WriteSignal<String>,
    _local_video_ref: NodeRef<leptos::html::Video>,
    _connection_errors: RwSignal<std::collections::HashSet<String>>,
) -> impl Fn(leptos::ev::MouseEvent) + 'static {
    move |_| {}
}

#[cfg(feature = "hydrate")]
fn share_toggle_handler(
    conn: RoomConnection,
    members: ReadSignal<Vec<RoomMember>>,
    my_peer_id: ReadSignal<Option<String>>,
    is_sharing: ReadSignal<bool>,
    set_is_sharing: WriteSignal<bool>,
    set_status: WriteSignal<String>,
    local_video_ref: NodeRef<leptos::html::Video>,
    connection_errors: RwSignal<std::collections::HashSet<String>>,
) -> impl Fn(leptos::ev::MouseEvent) + 'static {
    use leptos::task::spawn_local;
    use wasm_bindgen::JsCast;
    use web_sys::{MediaStreamTrack, RtcPeerConnectionIceEvent};

    use crate::client::webrtc::{capture_display, create_offer, new_peer_connection};
    use crate::signaling::protocol::ClientMessage;

    move |_| {
        if is_sharing.get_untracked() {
            stop_sharing(&conn, set_is_sharing);
            return;
        }

        let conn = conn.clone();
        set_status.set("Selecione a tela para compartilhar...".to_string());

        spawn_local(async move {
            let stream = match capture_display().await {
                Ok(stream) => stream,
                Err(_) => {
                    set_status.set("Conectado.".to_string());
                    return;
                }
            };

            if let Some(video) = local_video_ref.get_untracked() {
                video.set_src_object(Some(&stream));
                let _ = video.play();
            }
            *conn.local_stream.borrow_mut() = Some(stream.clone());
            set_is_sharing.set(true);

            // O botão nativo "Stop sharing" da barra de captura do navegador
            // também precisa disparar a mesma limpeza — sem isso, quem está
            // assistindo fica com a última imagem congelada.
            if let Ok(track) = stream.get_tracks().get(0).dyn_into::<MediaStreamTrack>() {
                let conn_for_end = conn.clone();
                let onended = wasm_bindgen::prelude::Closure::<dyn FnMut()>::new(move || {
                    stop_sharing(&conn_for_end, set_is_sharing);
                });
                track.set_onended(Some(onended.as_ref().unchecked_ref()));
                onended.forget();
            }

            if let Some(ws) = conn.ws.borrow().as_ref() {
                ws.send(&ClientMessage::StartShare);
            }

            let Some(my_id) = my_peer_id.get_untracked() else { return };

            // A ordem importa: StartShare precisa sair antes das ofertas (acima)
            // para que o PeerStartedSharing chegue em cada espectador antes do
            // Offer correspondente — é o que garante que o tile <video> já
            // exista no DOM quando o ontrack tentar encontrá-lo (Step 2).
            for member in members.get_untracked() {
                if member.peer_id == my_id {
                    continue;
                }
                let viewer_id = member.peer_id.clone();
                let conn = conn.clone();
                let my_id = my_id.clone();

                spawn_local(async move {
                    let Ok(pc) = new_peer_connection() else { return };
                    conn.outgoing.borrow_mut().insert(viewer_id.clone(), pc.clone());
                    connection_errors.update(|errors| { errors.remove(&viewer_id); });

                    if let Some(stream) = conn.local_stream.borrow().as_ref() {
                        for track in stream.get_tracks().iter() {
                            let track: MediaStreamTrack = track.unchecked_into();
                            pc.add_track_0(&track, stream);
                        }
                    }

                    let target_id = viewer_id.clone();
                    let conn_for_ice = conn.clone();
                    let my_id_for_ice = my_id.clone();
                    let onicecandidate = wasm_bindgen::prelude::Closure::<dyn FnMut(RtcPeerConnectionIceEvent)>::new(move |event: RtcPeerConnectionIceEvent| {
                        if let Some(candidate) = event.candidate() {
                            if let Some(ws) = conn_for_ice.ws.borrow().as_ref() {
                                ws.send(&ClientMessage::IceCandidate {
                                    to: target_id.clone(),
                                    stream_owner: my_id_for_ice.clone(),
                                    candidate: candidate.candidate(),
                                    sdp_mid: candidate.sdp_mid(),
                                    sdp_m_line_index: candidate.sdp_m_line_index(),
                                });
                            }
                        }
                    });
                    pc.set_onicecandidate(Some(onicecandidate.as_ref().unchecked_ref()));
                    onicecandidate.forget();

                    // Mesmo princípio de isolamento do lado de quem assiste
                    // (Step 2): se a conexão com ESSE espectador falhar, só o
                    // tile dele (do lado dele) fica com erro — não afeta os
                    // outros espectadores da minha transmissão.
                    let failed_viewer_id = viewer_id.clone();
                    let oniceconnectionstatechange = {
                        let pc_for_state = pc.clone();
                        wasm_bindgen::prelude::Closure::<dyn FnMut()>::new(move || {
                            if pc_for_state.ice_connection_state() == web_sys::RtcIceConnectionState::Failed {
                                connection_errors.update(|errors| { errors.insert(failed_viewer_id.clone()); });
                            }
                        })
                    };
                    pc.set_oniceconnectionstatechange(Some(oniceconnectionstatechange.as_ref().unchecked_ref()));
                    oniceconnectionstatechange.forget();

                    if let Ok(sdp) = create_offer(&pc).await {
                        if let Some(ws) = conn.ws.borrow().as_ref() {
                            ws.send(&ClientMessage::Offer { to: viewer_id, sdp });
                        }
                    }
                });
            }
        });
    }
}

#[cfg(feature = "hydrate")]
fn stop_sharing(conn: &RoomConnection, set_is_sharing: WriteSignal<bool>) {
    use wasm_bindgen::JsCast;

    if let Some(stream) = conn.local_stream.borrow_mut().take() {
        for track in stream.get_tracks().iter() {
            let track: web_sys::MediaStreamTrack = track.unchecked_into();
            track.stop();
        }
    }
    for (_, pc) in conn.outgoing.borrow_mut().drain() {
        pc.close();
    }
    if let Some(ws) = conn.ws.borrow().as_ref() {
        ws.send(&crate::signaling::protocol::ClientMessage::StopShare);
    }
    set_is_sharing.set(false);
}
```

- [ ] **Step 5: Atualizar o corpo de `RoomPage` para usar `RoomConnection`, o botão e os tiles de vídeo**

Substitua o começo do componente `RoomPage` (do `let (nick, ...)` até a chamada de `adopt_pending_session`) por:

```rust
    let (nick, set_nick) = signal(initial_nick());
    let (password, set_password) = signal(String::new());
    let (status, set_status) = signal("Informe o nick e a senha da sala.".to_string());
    let (authenticated, set_authenticated) = signal(false);
    let (members, set_members) = signal(Vec::<RoomMember>::new());
    let (my_peer_id, set_my_peer_id) = signal(None::<String>);
    let (is_sharing, set_is_sharing) = signal(false);
    let local_video_ref = NodeRef::<leptos::html::Video>::new();
    let connection_errors = RwSignal::new(std::collections::HashSet::<String>::new());
    let can_share = share_supported();

    let conn = RoomConnection::new();

    let join_room = setup_room_connection(
        initial_code.clone(),
        conn.clone(),
        set_status,
        set_authenticated,
        set_members,
        set_my_peer_id,
        connection_errors,
    );

    adopt_pending_session(initial_code, conn.clone(), set_status, set_authenticated, set_members, set_my_peer_id, connection_errors);
```

(Isso troca as chamadas da Task 7 — `setup_room_connection(initial_code, conn, set_status, ...)` e `adopt_pending_session(initial_code, conn, set_status, ...)` — pelas versões com `connection_errors` como último argumento, e troca `_my_peer_id`/`_set_my_peer_id` por `my_peer_id`/`set_my_peer_id` já que agora são usados de verdade.)

Logo depois de `let manual_join = { ... };`, adicione:

```rust
    let toggle_share = share_toggle_handler(
        conn,
        members,
        my_peer_id,
        is_sharing,
        set_is_sharing,
        set_status,
        local_video_ref,
        connection_errors,
    );
```

Substitua o `<div class="stage-header">...</div>` e o `<div class="grid">...</div>` dentro da `<div class="room-page" class:hidden=move || !authenticated.get()>` (a Task 7 não usa `<Show>` para essa alternância — ver a nota sobre `Send + Sync` no Step 1 da Task 7) por:

```rust
                <div class="stage-header">
                    <span class=lamp_class></span>
                    <span class="status-row__meta">{code}</span>
                    <span class="status-row__spacer"></span>
                    <Show when=move || can_share>
                        <button
                            class=move || if is_sharing.get() { "btn btn--danger" } else { "btn btn--primary" }
                            on:click=toggle_share
                        >
                            {move || if is_sharing.get() { "Parar de compartilhar" } else { "Compartilhar minha tela" }}
                        </button>
                    </Show>
                    <Show when=move || !can_share>
                        <span class="status-text status-text--error">
                            "Seu navegador não suporta compartilhar tela — você ainda pode assistir."
                        </span>
                    </Show>
                </div>
                <div class="grid">
                    <Show when=move || is_sharing.get()>
                        <div class="tile tile--self">
                            <video node_ref=local_video_ref autoplay=true playsinline=true muted=true></video>
                            <div class="tile__label">"Você (preview)"</div>
                        </div>
                    </Show>
                    <For
                        each=move || {
                            let my_id = my_peer_id.get();
                            members.get().into_iter().filter(move |m| m.sharing && Some(&m.peer_id) != my_id.as_ref()).collect::<Vec<_>>()
                        }
                        key=|m| m.peer_id.clone()
                        let(member)
                    >
                        <div class="tile">
                            <Show
                                when={
                                    let peer_id = member.peer_id.clone();
                                    move || !connection_errors.get().contains(&peer_id)
                                }
                                fallback=|| view! { <div class="tile__error">"Não foi possível conectar."</div> }
                            >
                                <video id=format!("video-{}", member.peer_id) autoplay=true playsinline=true></video>
                            </Show>
                            <div class="tile__label">{member.nick.clone()}</div>
                        </div>
                    </For>
                </div>
                <Show when=move || !members.get().iter().any(|m| m.sharing) && !is_sharing.get()>
                    <p class="status-text">"Ninguém está compartilhando a tela agora."</p>
                </Show>
```

- [ ] **Step 6: Verificar manualmente no navegador (ponta a ponta)**

Run: `cargo leptos watch`
1. Aba 1: crie a sala (nick "Ana", senha "teste123").
2. Aba 2: entre na mesma sala (nick "Bia", senha certa).
3. Na aba 1, clique "Compartilhar minha tela" e escolha uma janela — Expected: aba 1 mostra o preview local; aba 2 mostra um tile novo com o vídeo da Ana em poucos segundos.
4. Na aba 2, clique "Compartilhar minha tela" também — Expected: agora as duas abas mostram dois tiles cada (a própria pessoa como preview + a tela da outra pessoa).
5. Clique "Parar de compartilhar" na aba 1 — Expected: o tile da Ana desativa nas duas abas; a aba 2 continua compartilhando normalmente.
6. Feche a aba 1 (Ana sai) — Expected: aba 2 continua na sala sozinha (sala não morreu).
7. Feche a aba 2 — reabra `/r/<mesmo código>` numa aba nova e tente entrar — Expected: "Sala não encontrada ou já foi encerrada." (a sala morreu quando o último membro saiu).
8. Num navegador ou contexto sem `getDisplayMedia` (ex.: aba anônima com a API desabilitada, ou usando as ferramentas do desenvolvedor pra apagar `navigator.mediaDevices.getDisplayMedia` antes de carregar a página), entre numa sala — Expected: sem botão "Compartilhar minha tela", aparece o aviso "Seu navegador não suporta compartilhar tela — você ainda pode assistir."; a pessoa continua vendo normalmente as transmissões dos outros membros.

- [ ] **Step 7: Commit**

```bash
git add src/pages/room.rs
git commit -m "feat: implement multi-sharer WebRTC mesh with self-preview and video grid"
```

---

## Task 9: Estilos — formulários, grade e tiles

**Files:**
- Modify: `style/main.css`

**Interfaces:** nenhuma (CSS puro).

- [ ] **Step 1: Adicionar estilos de formulário (nick/senha) e de grade/tiles**

No final de `style/main.css`, adicione:

```css
/* --- formulários (criar sala / entrar na sala) --- */

.field {
  display: block;
  margin-bottom: 1rem;
}

.field__label {
  display: block;
  font-size: 0.78rem;
  color: var(--text-dim);
  margin-bottom: 0.4rem;
}

.field__input {
  width: 100%;
  background: var(--surface-2);
  border: 1px solid var(--border);
  border-radius: 0.6rem;
  padding: 0.65rem 0.8rem;
  font-family: inherit;
  font-size: 0.95rem;
  color: var(--text);
}

.field__input:focus-visible {
  outline: 2px solid var(--accent-strong);
  outline-offset: 1px;
}

/* --- sala: cabeçalho + grade de transmissões --- */

.room-page {
  width: 100%;
  height: 100vh;
  display: flex;
  flex-direction: column;
}

.room-page .stage-header {
  flex-wrap: wrap;
}

.room-page .stage-header .btn {
  width: auto;
  padding: 0.5rem 1rem;
  font-size: 0.85rem;
}

.grid {
  flex: 1 1 auto;
  min-height: 0;
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(min(100%, 22rem), 1fr));
  gap: 0.75rem;
  padding: 0.75rem;
  overflow-y: auto;
  background: #000;
}

.tile {
  position: relative;
  background: #111;
  border-radius: 0.6rem;
  overflow: hidden;
  aspect-ratio: 16 / 9;
}

.tile video {
  width: 100%;
  height: 100%;
  object-fit: contain;
  background: #000;
}

.tile__label {
  position: absolute;
  left: 0.5rem;
  bottom: 0.5rem;
  padding: 0.25rem 0.6rem;
  border-radius: 0.4rem;
  background: rgba(0, 0, 0, 0.55);
  color: var(--text);
  font-size: 0.78rem;
}

.tile--self {
  border: 1px solid var(--accent);
}

.tile__error {
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  text-align: center;
  padding: 1rem;
  color: var(--error);
  font-size: 0.85rem;
}
```

- [ ] **Step 2: Verificar manualmente no navegador**

Run: `cargo leptos watch`, revise as telas de "Criar sala", "Entrar na sala" e a grade de tiles com 2+ transmissões ativas (repetindo o fluxo da Task 8, Step 5). Expected: formulários legíveis, tiles em grade responsiva, sem overflow horizontal na janela.

- [ ] **Step 3: Commit**

```bash
git add style/main.css
git commit -m "style: add form and video-grid styles for multi-sharer rooms"
```

---

## Task 10: Documentação — `CLAUDE.md` e `README.md`

**Files:**
- Modify: `CLAUDE.md`
- Modify: `README.md`

**Interfaces:** nenhuma (documentação).

- [ ] **Step 1: Atualizar `CLAUDE.md`**

Na seção "## What this project is", troque o parágrafo que descreve o modelo 1-para-N por uma descrição do modelo de sala persistente e multiusuário: uma sala com ID e senha, onde qualquer participante entra com nick + senha, pode compartilhar sua tela a qualquer momento, e todos veem simultaneamente as transmissões ativas dos outros numa grade. Mantenha a menção de que não há áudio (ainda fora de escopo) e que é tudo em navegador (Windows/Linux), sem instalar nada.

Na seção "### Room lifecycle", substitua a descrição do modelo host/viewer por: qualquer membro pode iniciar/parar seu compartilhamento a qualquer momento; a sala é identificada por um código com senha (hash `argon2`, verificada no servidor); a sala só é removida do registro quando o último membro sai — sair de quem criou não afeta os demais; não existe hierarquia entre participantes.

- [ ] **Step 2: Atualizar `README.md`**

No parágrafo de abertura, troque "Site para compartilhar a tela com até 5 amigos ao mesmo tempo... sem contas" pela descrição do v2 (sala com senha, até 8 pessoas, nick salvo localmente, qualquer um pode compartilhar).

Substitua a seção "## Checklist de teste manual (fluxo completo)" pelos passos da Task 8, Step 5 e Task 7, Step 2 deste plano (criar sala, entrar com senha errada/certa, compartilhamento simultâneo de duas pessoas, sala sobrevivendo à saída do criador, sala sumindo quando o último membro sai).

Na seção "## Deploy (geral)", mantenha a observação de que não há banco de dados — mas ajuste a frase sobre "estado de salas" para deixar explícito que isso agora inclui o hash de senha de cada sala (também descartado num restart, como o resto do estado em memória).

- [ ] **Step 3: Commit**

```bash
git add CLAUDE.md README.md
git commit -m "docs: update CLAUDE.md and README for persistent multi-sharer rooms"
```
