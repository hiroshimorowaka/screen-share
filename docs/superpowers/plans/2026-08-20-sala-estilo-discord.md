# Sala estilo Discord — Plano de Implementação

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Transformar a sala persistente multiusuário (v2) numa experiência estilo
Discord: nome de sala obrigatório e compartilhado, salas recentes lembradas no
navegador com verificação imediata de existência, identidade visual (nick + cor +
avatar), compartilhamento assistido sob demanda (em vez de automático pra todo
mundo), e limite de 10 membros por sala.

**Architecture:** Estende o protocolo de sinalização existente (mais campos e duas
mensagens novas, roteadas pelo mesmo mecanismo de relay já usado por
Offer/Answer/IceCandidate) e adiciona um endpoint HTTP simples fora do WebSocket para
checagem de existência de sala. No cliente, a grade de vídeo vira uma grade de
"cards" de membro — construída como uma lista fixa de `MAX_MEMBERS` (10) fragmentos
estáticos, não uma `<For>` reativa, porque os botões de cada card (assistir, parar
de assistir) precisam capturar a conexão WebSocket (`Rc<RefCell<...>>`, não
`Send + Sync`) e o Leptos 0.8 exige `Send + Sync` de qualquer closure de filho
dinâmico usada por `<For>`/`<Show>` — o mesmo motivo que já forçou `class:hidden` no
lugar de `<Show>` em várias partes do v2. Nenhum banco de dados é necessário: nome
de sala vive no registro em memória do servidor (mesma vida útil da sala), e
identidade + salas recentes vivem só no `localStorage` de cada navegador.

**Tech Stack:** Rust, Leptos 0.8 (SSR + hydrate), Axum, Tokio, `serde`/`serde_json`,
`web-sys`/`wasm-bindgen` para `fetch` e WebRTC, `reqwest` (novo, só em
`dev-dependencies`, pra testar o endpoint HTTP novo).

## Global Constraints

- Este plano **estende** o v2, já implementado nesta branch — todo padrão
  arquitetural do v2 continua valendo: par de funções `#[cfg(feature = "hydrate")]`
  / `#[cfg(not(feature = "hydrate"))]` para qualquer coisa que toque `web-sys`;
  `class:hidden` (nunca `<Show>`) para alternar qualquer elemento cujo `on:click`
  capture algo que carregue `WsClient`/`RoomConnection`; `cargo test --features ssr`
  e `cargo check --features hydrate --target wasm32-unknown-unknown --lib` são os
  dois comandos de verificação depois de qualquer mudança em `src/`.
- `MAX_MEMBERS` sobe de 8 para **10** — vira uma constante única em
  `src/signaling/protocol.rs`, usada tanto pelo servidor (capacidade) quanto pelo
  cliente (quantidade de slots de card renderizados).
- Sem contas de usuário, sem senha persistida no navegador (decisão explícita:
  entrar numa sala recente ainda pede a senha).
- Referência de design completa: `docs/superpowers/specs/2026-08-20-sala-estilo-discord-design.md`.
  Este plano não repete o "porquê" de cada decisão — só o "como".

---

## Task 1: Protocolo — cor, nome de sala, status HTTP e mensagens de assistir

**Files:**
- Modify: `src/signaling/protocol.rs`

**Interfaces:**
- Produces: `protocol::MAX_MEMBERS: usize = 10` (usado pelas Tasks 2 e 8).
  `protocol::RoomStatus { exists: bool, name: Option<String>, member_count: Option<usize> }`
  (usado pelas Tasks 3 e 6). Campos novos em `MemberInfo`, `ClientMessage`,
  `ServerMessage` (usados por todas as tasks seguintes que tocam o protocolo).

- [ ] **Step 1: Atualizar os testes existentes pros novos campos obrigatórios**

Em `src/signaling/protocol.rs`, dentro do `mod tests`, atualize os três testes que
constroem `ClientMessage`/`ServerMessage` — eles não vão compilar assim que os
campos novos forem adicionados na Step 3, então isso conta como "escrever o teste
que falha" (falha por erro de compilação, não de asserção):

```rust
#[test]
fn create_room_message_round_trips_through_json() {
    let msg = ClientMessage::CreateRoom {
        nick: "Ana".to_string(),
        password: "abacate".to_string(),
        room_name: "Sala dos lindos".to_string(),
        color: "coral".to_string(),
    };
    let json = serde_json::to_string(&msg).unwrap();
    assert_eq!(
        json,
        r#"{"type":"create_room","nick":"Ana","password":"abacate","room_name":"Sala dos lindos","color":"coral"}"#
    );

    let parsed: ClientMessage = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, msg);
}

#[test]
fn join_room_message_round_trips_through_json() {
    let msg = ClientMessage::JoinRoom {
        room: "ABCD1234".to_string(),
        nick: "Bia".to_string(),
        password: "abacate".to_string(),
        color: "sky".to_string(),
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
        room_name: "Sala dos lindos".to_string(),
        members: vec![MemberInfo {
            peer_id: "peer-1".to_string(),
            nick: "Ana".to_string(),
            color: "coral".to_string(),
        }],
        active_sharers: vec![],
    };
    let json = serde_json::to_string(&msg).unwrap();
    let parsed: ServerMessage = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, msg);
}
```

Adicione também dois testes novos, no mesmo `mod tests`:

```rust
#[test]
fn watch_share_message_round_trips_through_json() {
    let msg = ClientMessage::WatchShare { sharer_id: "peer-1".to_string() };
    let json = serde_json::to_string(&msg).unwrap();
    assert_eq!(json, r#"{"type":"watch_share","sharer_id":"peer-1"}"#);

    let parsed: ClientMessage = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, msg);
}

#[test]
fn room_status_omits_absent_fields_when_room_does_not_exist() {
    let status = RoomStatus { exists: false, name: None, member_count: None };
    let json = serde_json::to_string(&status).unwrap();
    assert_eq!(json, r#"{"exists":false}"#);

    let parsed: RoomStatus = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, status);
}
```

- [ ] **Step 2: Rodar os testes e confirmar que falham (erro de compilação)**

Run: `cargo test --features ssr --lib signaling::protocol`
Expected: FAIL — `error[E0063]: missing fields room_name, color in initializer of ClientMessage::CreateRoom` (e equivalentes para os outros campos novos).

- [ ] **Step 3: Implementar os campos e mensagens novas**

Substitua o conteúdo de `src/signaling/protocol.rs` até o início do `mod tests` por:

```rust
use serde::{Deserialize, Serialize};

pub const MAX_MEMBERS: usize = 10;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MemberInfo {
    pub peer_id: String,
    pub nick: String,
    pub color: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    CreateRoom { nick: String, password: String, room_name: String, color: String },
    JoinRoom { room: String, nick: String, password: String, color: String },
    StartShare,
    StopShare,
    WatchShare { sharer_id: String },
    StopWatching { sharer_id: String },
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
        room_name: String,
        members: Vec<MemberInfo>,
        active_sharers: Vec<String>,
    },
    AuthFailed,
    RoomNotFound,
    RoomFull,
    PeerJoined { peer_id: String, nick: String, color: String },
    PeerLeft { peer_id: String },
    PeerStartedSharing { peer_id: String },
    PeerStoppedSharing { peer_id: String },
    WatchRequested { from: String },
    WatchStopped { from: String },
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

/// Resposta do endpoint `GET /api/rooms/:code` (fora do WebSocket) — permite
/// checar se uma sala existe (e ver seu nome) sem abrir conexão nem digitar
/// senha. Quando `exists` é `false`, os outros dois campos são omitidos do
/// JSON (não seriam relevantes).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RoomStatus {
    pub exists: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub member_count: Option<usize>,
}
```

- [ ] **Step 4: Rodar os testes e confirmar que passam**

Run: `cargo test --features ssr --lib signaling::protocol`
Expected: PASS (6 testes: os 4 originais atualizados + os 2 novos).

- [ ] **Step 5: Commit**

```bash
git add src/signaling/protocol.rs
git commit -m "feat: add color, room name, watch requests, and room status to the protocol"
```

---

## Task 2: Registro de salas — nome, cor dos membros e status para o endpoint HTTP

**Files:**
- Modify: `src/signaling/registry.rs`

**Interfaces:**
- Consumes: `protocol::{MemberInfo, ServerMessage, MAX_MEMBERS}` (Task 1).
- Produces: `Registry::create_room(nick, color, room_name, password, sender)`,
  `Registry::join_room(room_code, nick, color, password, sender)` (assinaturas
  novas, usadas pela Task 4). `Registry::room_status(room_code) -> Option<RoomSummary>`
  e `pub struct RoomSummary { pub name: String, pub member_count: usize }` (usados
  pela Task 3). `JoinedSnapshot` ganha o campo `room_name: String`.

- [ ] **Step 1: Atualizar os testes existentes pra nova assinatura**

Em `src/signaling/registry.rs`, no `mod tests`, toda chamada a `create_room`/
`join_room` precisa de dois argumentos novos (`color`, e `room_name` só em
`create_room`, entre `nick` e `password`). Atualize todas as chamadas existentes,
por exemplo:

```rust
let (room_code, snapshot) = registry.create_room(
    "Ana".to_string(),
    "coral".to_string(),
    "Sala da Ana".to_string(),
    "senha123",
    tx,
);
```

e

```rust
let result = registry.join_room(&room_code, "Bia".to_string(), "sky".to_string(), "senha-errada", viewer_tx);
```

Aplique esse padrão (adicionando `"coral".to_string()` — ou qualquer cor válida —
como segundo argumento de `create_room`/`join_room`, e `"Sala ...".to_string()`
como terceiro argumento só de `create_room`) em todas as ocorrências dos dois
métodos no arquivo, incluindo dentro do loop de `join_room_full_returns_error`.

Atualize também a asserção de `create_room_registers_creator_and_returns_snapshot`
para incluir `color`:

```rust
assert_eq!(
    snapshot.members,
    vec![MemberInfo { peer_id: snapshot.peer_id.clone(), nick: "Ana".to_string(), color: "coral".to_string() }]
);
assert_eq!(snapshot.room_name, "Sala da Ana");
```

E a asserção de `join_room_success_notifies_existing_members_and_includes_them_in_snapshot`:

```rust
let notification = host_rx.recv().await.unwrap();
assert_eq!(
    notification,
    ServerMessage::PeerJoined { peer_id: snapshot.peer_id.clone(), nick: "Bia".to_string(), color: "sky".to_string() }
);
```

Adicione também um teste novo, cobrindo o endpoint de status:

```rust
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
```

- [ ] **Step 2: Rodar os testes e confirmar que falham (erro de compilação)**

Run: `cargo test --features ssr --lib signaling::registry`
Expected: FAIL — assinatura de `create_room`/`join_room` não bate com as chamadas
atualizadas, e `room_status` ainda não existe.

- [ ] **Step 3: Implementar**

Em `src/signaling/registry.rs`:

```rust
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
```

(Mantenha o `mod tests` já atualizado na Step 1 — só a parte acima do módulo de
testes muda.) A única mudança de comportamento real é a fonte de `MAX_MEMBERS`
(agora `protocol::MAX_MEMBERS = 10`, não mais uma constante própria de 8) e os
campos novos de `name`/`color` passando pelas mesmas rotas que já existiam.

- [ ] **Step 4: Rodar os testes e confirmar que passam**

Run: `cargo test --features ssr --lib signaling::registry`
Expected: PASS — todos os testes, incluindo os 2 novos de `room_status` e a
capacidade efetiva de 10 (o teste `join_room_full_returns_error` já usa
`1..MAX_MEMBERS`, então automaticamente passa a testar contra 10 sem precisar
editar o valor).

- [ ] **Step 5: Commit**

```bash
git add src/signaling/registry.rs
git commit -m "feat: store room name and member colors, add room_status lookup"
```

---

## Task 3: Endpoint HTTP `GET /api/rooms/:code`

**Files:**
- Create: `src/signaling/rooms_status.rs`
- Modify: `src/signaling/mod.rs`
- Modify: `src/main.rs`
- Create: `tests/rooms_status.rs`
- Modify: `Cargo.toml` (adicionar `reqwest` em `[dev-dependencies]`)

**Interfaces:**
- Consumes: `Registry::room_status` (Task 2), `protocol::RoomStatus` (Task 1).
- Produces: `rooms_status::room_status_handler` — handler Axum registrado em
  `GET /api/rooms/{code}`, usado pela Task 6 (cliente) via `fetch`.

- [ ] **Step 1: Adicionar `reqwest` como dependência de desenvolvimento**

Em `Cargo.toml`, no bloco `[dev-dependencies]`:

```toml
[dev-dependencies]
tokio-tungstenite = "0.30.0"
reqwest = { version = "0.12", features = ["json"] }
```

- [ ] **Step 2: Escrever o teste de integração que falha**

`tests/rooms_status.rs`:

```rust
use axum::routing::get;
use axum::Router;
use screen_share::signaling::protocol::{ClientMessage, RoomStatus, ServerMessage};
use screen_share::signaling::registry::Registry;
use screen_share::signaling::rooms_status::room_status_handler;
use screen_share::signaling::ws::ws_handler;

async fn spawn_test_server() -> (String, String) {
    let registry = Registry::new();
    let app = Router::new()
        .route("/ws", get(ws_handler))
        .route("/api/rooms/{code}", get(room_status_handler))
        .with_state(registry);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app.into_make_service()).await.unwrap();
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
        password: "senha123".to_string(),
        room_name: "Sala dos lindos".to_string(),
        color: "coral".to_string(),
    };
    ws.send(Message::Text(serde_json::to_string(&create).unwrap().into())).await.unwrap();

    let room = match ws.next().await.unwrap().unwrap() {
        Message::Text(text) => match serde_json::from_str::<ServerMessage>(&text).unwrap() {
            ServerMessage::Joined { room, .. } => room,
            other => panic!("esperava Joined, recebeu {other:?}"),
        },
        other => panic!("mensagem inesperada: {other:?}"),
    };

    let status: RoomStatus = reqwest::get(format!("{http_url}/api/rooms/{room}")).await.unwrap().json().await.unwrap();
    assert_eq!(status, RoomStatus { exists: true, name: Some("Sala dos lindos".to_string()), member_count: Some(1) });
}

#[tokio::test]
async fn room_status_reports_missing_room_as_nonexistent() {
    let (_ws_url, http_url) = spawn_test_server().await;
    let status: RoomStatus = reqwest::get(format!("{http_url}/api/rooms/NOPE0000")).await.unwrap().json().await.unwrap();
    assert_eq!(status, RoomStatus { exists: false, name: None, member_count: None });
}
```

- [ ] **Step 3: Rodar os testes e confirmar que falham**

Run: `cargo test --test rooms_status`
Expected: FAIL — `error[E0433]: failed to resolve: could not find rooms_status in signaling` (o módulo ainda não existe).

- [ ] **Step 4: Implementar o handler**

`src/signaling/rooms_status.rs`:

```rust
use axum::extract::{Path, State};
use axum::Json;

use super::protocol::RoomStatus;
use super::registry::Registry;

pub async fn room_status_handler(State(registry): State<Registry>, Path(code): Path<String>) -> Json<RoomStatus> {
    match registry.room_status(&code) {
        Some(summary) => Json(RoomStatus {
            exists: true,
            name: Some(summary.name),
            member_count: Some(summary.member_count),
        }),
        None => Json(RoomStatus { exists: false, name: None, member_count: None }),
    }
}
```

Em `src/signaling/mod.rs`, adicione (junto dos outros módulos `ssr`):

```rust
#[cfg(feature = "ssr")]
pub mod rooms_status;
```

Em `src/main.rs`, registre a rota nova no mesmo router de sinalização:

```rust
use screen_share::signaling::rooms_status::room_status_handler;
```

(junto dos outros `use` já existentes) e troque:

```rust
let signaling_router = Router::new()
    .route("/ws", get(ws_handler))
    .with_state(signaling_state);
```

por:

```rust
let signaling_router = Router::new()
    .route("/ws", get(ws_handler))
    .route("/api/rooms/{code}", get(room_status_handler))
    .with_state(signaling_state);
```

> Axum 0.8 usa `{code}` (chaves) pra parâmetro de rota, não `:code` (dois-pontos) —
> a sintaxe antiga do Axum 0.7 não compila aqui.

- [ ] **Step 5: Rodar os testes e confirmar que passam**

Run: `cargo test --test rooms_status`
Expected: PASS — 2 testes.

Run também: `cargo test --features ssr` (suíte inteira) e
`cargo check --features ssr --bin screen_share`, pra confirmar que `main.rs`
compila com a rota nova.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml src/signaling/rooms_status.rs src/signaling/mod.rs src/main.rs tests/rooms_status.rs
git commit -m "feat: add GET /api/rooms/:code for room-existence checks"
```

---

## Task 4: WebSocket — repassar nome/cor e rotear pedidos de assistir

**Files:**
- Modify: `src/signaling/ws.rs`
- Modify: `tests/signaling_ws.rs`

**Interfaces:**
- Consumes: `Registry::{create_room, join_room, relay}` com as assinaturas novas
  (Task 2), `ClientMessage::{WatchShare, StopWatching}` /
  `ServerMessage::{WatchRequested, WatchStopped, Joined}` (Task 1).

- [ ] **Step 1: Atualizar os testes de integração existentes**

Em `tests/signaling_ws.rs`, toda construção de `ClientMessage::CreateRoom`/
`JoinRoom` precisa dos campos novos. Atualize `create_room_then_join_with_wrong_and_right_password`:

```rust
send_json(&mut creator_ws, &ClientMessage::CreateRoom {
    nick: "Ana".to_string(),
    password: "senha123".to_string(),
    room_name: "Sala da Ana".to_string(),
    color: "coral".to_string(),
})
.await;
```

```rust
send_json(&mut viewer_ws, &ClientMessage::JoinRoom {
    room: room.clone(),
    nick: "Bia".to_string(),
    password: "senha-errada".to_string(),
    color: "sky".to_string(),
})
.await;
```

(e a segunda tentativa, com a senha certa, ganha o mesmo `color: "sky".to_string()`).
A asserção final também ganha `color`:

```rust
assert_eq!(
    recv_json(&mut creator_ws).await,
    ServerMessage::PeerJoined { peer_id: viewer_id, nick: "Bia".to_string(), color: "sky".to_string() }
);
```

Faça o mesmo em `start_share_broadcasts_and_offer_is_relayed` (adicione
`room_name`/`color` no `CreateRoom` e `color` no `JoinRoom`).

Adicione um teste novo cobrindo o roteamento de `WatchShare`:

```rust
#[tokio::test]
async fn watch_share_is_relayed_only_to_the_sharer() {
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

    send_json(&mut viewer_ws, &ClientMessage::StopWatching { sharer_id }).await;
    assert_eq!(recv_json(&mut sharer_ws).await, ServerMessage::WatchStopped { from: viewer_id });
}
```

- [ ] **Step 2: Rodar os testes e confirmar que falham**

Run: `cargo test --test signaling_ws`
Expected: FAIL — assinaturas de `ClientMessage`/`ServerMessage` sem os campos
novos não compilam (a lógica de `ws.rs` ainda espera as mensagens antigas).

- [ ] **Step 3: Implementar**

Em `src/signaling/ws.rs`, troque o `match client_msg` inteiro por:

```rust
match client_msg {
    ClientMessage::CreateRoom { nick, password, room_name, color } => {
        let (code, snapshot) = registry.create_room(nick, color, room_name, &password, tx.clone());
        let _ = tx.send(ServerMessage::Joined {
            peer_id: snapshot.peer_id.clone(),
            room: code.clone(),
            room_name: snapshot.room_name,
            members: snapshot.members,
            active_sharers: snapshot.active_sharers,
        });
        room_code = Some(code);
        peer_id = Some(snapshot.peer_id);
    }
    ClientMessage::JoinRoom { room, nick, password, color } => {
        match registry.join_room(&room, nick, color, &password, tx.clone()) {
            Ok(snapshot) => {
                let _ = tx.send(ServerMessage::Joined {
                    peer_id: snapshot.peer_id.clone(),
                    room: room.clone(),
                    room_name: snapshot.room_name,
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
    ClientMessage::WatchShare { sharer_id } => {
        if let (Some(room), Some(from)) = (&room_code, &peer_id) {
            registry.relay(room, &sharer_id, ServerMessage::WatchRequested { from: from.clone() });
        }
    }
    ClientMessage::StopWatching { sharer_id } => {
        if let (Some(room), Some(from)) = (&room_code, &peer_id) {
            registry.relay(room, &sharer_id, ServerMessage::WatchStopped { from: from.clone() });
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
```

- [ ] **Step 4: Rodar os testes e confirmar que passam**

Run: `cargo test --features ssr`
Expected: PASS — suíte inteira (protocolo, registro, `signaling_ws`, `rooms_status`).

- [ ] **Step 5: Commit**

```bash
git add src/signaling/ws.rs tests/signaling_ws.rs
git commit -m "feat: relay room name/color and watch requests over the ws endpoint"
```

---

## Task 5: Paleta de cores fixa (compartilhada por home e sala)

**Files:**
- Create: `src/pages/palette.rs`
- Modify: `src/pages/mod.rs`

**Interfaces:**
- Produces: `palette::DEFAULT_COLOR: &str`, `palette::palette_ids() -> impl Iterator<Item = &'static str>`,
  `palette::color_hex(id: &str) -> (&'static str, &'static str)` (borda, fundo),
  `palette::avatar_letter(nick: &str) -> String` — usados pelas Tasks 6 (perfil
  padrão), 7 e 8 (formulários e cards).

- [ ] **Step 1: Escrever os testes que falham**

`src/pages/palette.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_hex_returns_the_pair_for_a_known_id() {
        assert_eq!(color_hex("coral"), ("#ff6b6b", "#3a1f1f"));
    }

    #[test]
    fn color_hex_falls_back_to_slate_for_an_unknown_id() {
        assert_eq!(color_hex("cor-que-nao-existe"), color_hex("slate"));
    }

    #[test]
    fn avatar_letter_uppercases_the_first_character() {
        assert_eq!(avatar_letter("ana"), "A");
        assert_eq!(avatar_letter("  bia"), "B");
    }

    #[test]
    fn avatar_letter_falls_back_to_question_mark_for_empty_nick() {
        assert_eq!(avatar_letter("   "), "?");
    }

    #[test]
    fn default_color_is_a_valid_palette_id() {
        assert!(palette_ids().any(|id| id == DEFAULT_COLOR));
    }
}
```

- [ ] **Step 2: Rodar os testes e confirmar que falham**

Run: `cargo test --features ssr --lib pages::palette`
Expected: FAIL — `error[E0433]: failed to resolve: could not find palette in pages` (módulo ainda não declarado/implementado).

- [ ] **Step 3: Implementar**

No topo de `src/pages/palette.rs` (antes do `mod tests` da Step 1):

```rust
/// Paleta fixa de cores de identificação de membro. Cada entrada é
/// (id enviado no protocolo, cor da borda/avatar, cor de fundo do card — a
/// mesma cor escurecida e com menos opacidade sobre o tema escuro do site).
const PALETTE: &[(&str, &str, &str)] = &[
    ("coral", "#ff6b6b", "#3a1f1f"),
    ("amber", "#ffb347", "#3a2a14"),
    ("gold", "#ffd93d", "#3a3316"),
    ("lime", "#a8e063", "#263a1a"),
    ("teal", "#4ecdc4", "#17322f"),
    ("sky", "#45b7ff", "#16283a"),
    ("periwinkle", "#7c83fd", "#23233f"),
    ("violet", "#b57cff", "#2c2140"),
    ("pink", "#ff6fb5", "#3a1f30"),
    ("slate", "#b0b8c1", "#2a2d31"),
];

pub const DEFAULT_COLOR: &str = "coral";

pub fn palette_ids() -> impl Iterator<Item = &'static str> {
    PALETTE.iter().map(|(id, _, _)| *id)
}

pub fn color_hex(id: &str) -> (&'static str, &'static str) {
    PALETTE
        .iter()
        .find(|(entry_id, _, _)| *entry_id == id)
        .map(|(_, border, bg)| (*border, *bg))
        .unwrap_or(("#b0b8c1", "#2a2d31"))
}

pub fn avatar_letter(nick: &str) -> String {
    nick.trim()
        .chars()
        .next()
        .map(|c| c.to_uppercase().to_string())
        .unwrap_or_else(|| "?".to_string())
}
```

Em `src/pages/mod.rs`, adicione:

```rust
pub mod palette;
```

- [ ] **Step 4: Rodar os testes e confirmar que passam**

Run: `cargo test --features ssr --lib pages::palette`
Expected: PASS — 5 testes.

- [ ] **Step 5: Commit**

```bash
git add src/pages/palette.rs src/pages/mod.rs
git commit -m "feat: add fixed member color palette and avatar-letter helper"
```

---

## Task 6: Estado do cliente — perfil (nick + cor), salas recentes e checagem HTTP

**Files:**
- Modify: `src/client/storage.rs`
- Create: `src/client/rooms_api.rs`
- Modify: `src/client/mod.rs`
- Modify: `Cargo.toml` (adicionar feature `"Response"` ao `web-sys`)

**Interfaces:**
- Consumes: `palette::DEFAULT_COLOR` (Task 5), `protocol::RoomStatus` (Task 1).
- Produces: `storage::{Profile, load_profile() -> Profile, save_profile(&Profile)}`,
  `storage::{RecentRoom, load_recent_rooms() -> Vec<RecentRoom>, save_recent_room(RecentRoom), remove_recent_room(code: &str)}`,
  `rooms_api::check_room(code: &str) -> impl Future<Output = Option<RoomStatus>>` —
  usados pelas Tasks 7 e 8. `storage::{load_nick, save_nick}` (do v2) continuam
  existindo por enquanto — a Task 8 é quem os remove, depois de migrar o último
  chamador.

- [ ] **Step 1: Adicionar a feature `Response` ao `web-sys`**

Em `Cargo.toml`, no bloco `web-sys = { ..., features = [...] }`, adicione
`"Response"` à lista (ex.: logo após `"Storage"`).

- [ ] **Step 2: Implementar `Profile` e salas recentes em `storage.rs`**

No topo de `src/client/storage.rs` (mantendo `load_nick`/`save_nick` como estão
hoje, logo abaixo), adicione:

```rust
use serde::{Deserialize, Serialize};

const PROFILE_KEY: &str = "screen_share_profile";
const RECENT_ROOMS_KEY: &str = "screen_share_recent_rooms";
const MAX_RECENT_ROOMS: usize = 10;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Profile {
    pub nick: String,
    pub color: String,
}

impl Default for Profile {
    fn default() -> Self {
        Self { nick: String::new(), color: crate::pages::palette::DEFAULT_COLOR.to_string() }
    }
}

#[cfg(not(feature = "hydrate"))]
pub fn load_profile() -> Profile {
    Profile::default()
}

#[cfg(feature = "hydrate")]
pub fn load_profile() -> Profile {
    let Some(window) = web_sys::window() else { return Profile::default() };
    let Ok(Some(storage)) = window.local_storage() else { return Profile::default() };
    let Ok(Some(json)) = storage.get_item(PROFILE_KEY) else { return Profile::default() };
    serde_json::from_str(&json).unwrap_or_default()
}

#[cfg(not(feature = "hydrate"))]
pub fn save_profile(_profile: &Profile) {}

#[cfg(feature = "hydrate")]
pub fn save_profile(profile: &Profile) {
    if let Some(window) = web_sys::window() {
        if let Ok(Some(storage)) = window.local_storage() {
            if let Ok(json) = serde_json::to_string(profile) {
                let _ = storage.set_item(PROFILE_KEY, &json);
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RecentRoom {
    pub code: String,
    pub name: String,
}

#[cfg(not(feature = "hydrate"))]
pub fn load_recent_rooms() -> Vec<RecentRoom> {
    Vec::new()
}

#[cfg(feature = "hydrate")]
pub fn load_recent_rooms() -> Vec<RecentRoom> {
    let Some(window) = web_sys::window() else { return Vec::new() };
    let Ok(Some(storage)) = window.local_storage() else { return Vec::new() };
    let Ok(Some(json)) = storage.get_item(RECENT_ROOMS_KEY) else { return Vec::new() };
    serde_json::from_str(&json).unwrap_or_default()
}

#[cfg(not(feature = "hydrate"))]
pub fn save_recent_room(_room: RecentRoom) {}

#[cfg(feature = "hydrate")]
pub fn save_recent_room(room: RecentRoom) {
    let mut rooms = load_recent_rooms();
    rooms.retain(|r| r.code != room.code);
    rooms.insert(0, room);
    rooms.truncate(MAX_RECENT_ROOMS);
    save_recent_rooms_list(&rooms);
}

#[cfg(not(feature = "hydrate"))]
pub fn remove_recent_room(_code: &str) {}

#[cfg(feature = "hydrate")]
pub fn remove_recent_room(code: &str) {
    let mut rooms = load_recent_rooms();
    rooms.retain(|r| r.code != code);
    save_recent_rooms_list(&rooms);
}

#[cfg(feature = "hydrate")]
fn save_recent_rooms_list(rooms: &[RecentRoom]) {
    if let Some(window) = web_sys::window() {
        if let Ok(Some(storage)) = window.local_storage() {
            if let Ok(json) = serde_json::to_string(rooms) {
                let _ = storage.set_item(RECENT_ROOMS_KEY, &json);
            }
        }
    }
}
```

- [ ] **Step 3: Implementar o helper de checagem HTTP**

`src/client/rooms_api.rs`:

```rust
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::Response;

use crate::signaling::protocol::RoomStatus;

/// Consulta `GET /api/rooms/:code`. Retorna `None` em qualquer falha de rede
/// ou de parsing — quem chama trata isso como "inconclusivo", não como "a
/// sala não existe" (só um `RoomStatus { exists: false, .. }` de verdade
/// significa isso).
pub async fn check_room(code: &str) -> Option<RoomStatus> {
    let window = web_sys::window()?;
    let promise = window.fetch_with_str(&format!("/api/rooms/{code}"));
    let resp_value = JsFuture::from(promise).await.ok()?;
    let response: Response = resp_value.dyn_into().ok()?;
    let text_promise = response.text().ok()?;
    let text_value = JsFuture::from(text_promise).await.ok()?;
    let text = text_value.as_string()?;
    serde_json::from_str(&text).ok()
}
```

Em `src/client/mod.rs`, adicione:

```rust
pub mod rooms_api;
```

- [ ] **Step 4: Verificar que compila nos dois alvos**

Run: `cargo check --features ssr --bin screen_share`
Expected: sucesso (o módulo `client` inteiro é `#[cfg(feature = "hydrate")]` em
`src/lib.rs`, então nem entra na compilação `ssr`).

Run: `cargo check --features hydrate --target wasm32-unknown-unknown --lib`
Expected: sucesso.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml src/client/storage.rs src/client/rooms_api.rs src/client/mod.rs
git commit -m "feat: add profile/recent-rooms storage and a room-existence fetch helper"
```

---

## Task 7: Página inicial — nome da sala, cor, salas recentes

**Files:**
- Modify: `src/pages/home.rs`
- Modify: `src/client/session.rs` (adicionar `room_name` a `PendingSession`)

**Interfaces:**
- Consumes: `client::storage::{Profile, load_profile, save_profile, RecentRoom, load_recent_rooms, save_recent_room, remove_recent_room}`,
  `client::rooms_api::check_room` (Task 6), `client::session::{self, PendingSession}`,
  `pages::palette::{palette_ids, color_hex}` (Task 5), `protocol::{ClientMessage, ServerMessage}` (Task 1).
- Produces: `PendingSession.room_name: String` (novo campo, consumido pela Task 8).

- [ ] **Step 1: Adicionar `room_name` a `PendingSession`**

Em `src/client/session.rs`, adicione o campo `room_name` à struct (mantendo o
resto do arquivo — `thread_local!`, `stash`, `take` — sem mudanças):

```rust
pub struct PendingSession {
    pub room: String,
    pub room_name: String,
    pub ws: WsClient,
    pub peer_id: String,
    pub members: Vec<MemberInfo>,
    pub active_sharers: Vec<String>,
}
```

- [ ] **Step 2: Substituir `src/pages/home.rs`**

```rust
use leptos::prelude::*;

use crate::pages::palette::{color_hex, palette_ids};
use crate::pages::status::status_meta;

#[component]
pub fn HomePage() -> impl IntoView {
    let profile = initial_profile();
    let (nick, set_nick) = signal(profile.nick);
    let (color, set_color) = signal(profile.color);
    let (room_name, set_room_name) = signal(String::new());
    let (password, set_password) = signal(String::new());
    let (status, set_status) = signal("Pronto para criar uma sala.".to_string());
    let (submitting, set_submitting) = signal(false);
    let (recent_rooms, set_recent_rooms) = signal(initial_recent_rooms());

    prune_recent_rooms(set_recent_rooms);

    let create_room = create_room_handler(nick, color, room_name, password, set_status, set_submitting);

    view! {
        <div class="panel">
            <h1>"Criar sala"</h1>
            <p class="subtext">"Escolha um nick, uma cor, um nome e uma senha. Compartilhe o link e a senha com quem você quiser na sala."</p>

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
                <div class="field">
                    <span class="field__label">"Sua cor"</span>
                    <div class="color-picker">
                        {palette_ids()
                            .map(|id| {
                                let (border, _) = color_hex(id);
                                view! {
                                    <button
                                        type="button"
                                        class="color-swatch"
                                        class:color-swatch--selected=move || color.get() == id
                                        style=format!("background-color: {border}")
                                        on:click=move |_| set_color.set(id.to_string())
                                    ></button>
                                }
                            })
                            .collect::<Vec<_>>()}
                    </div>
                </div>
                <label class="field">
                    <span class="field__label">"Nome da sala"</span>
                    <input
                        class="field__input"
                        type="text"
                        required
                        prop:value=room_name
                        on:input:target=move |ev| set_room_name.set(ev.target().value())
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

            <div class="recent-rooms" class:hidden=move || recent_rooms.get().is_empty()>
                <p class="invite__label">"Salas recentes"</p>
                <For each=move || recent_rooms.get() key=|r| r.code.clone() let(room)>
                    <a class="recent-room" href=format!("/r/{}", room.code)>
                        <span class="recent-room__name">{room.name.clone()}</span>
                        <span class="recent-room__code">{room.code.clone()}</span>
                    </a>
                </For>
            </div>
        </div>
    }
}

fn initial_profile() -> crate::client::storage::Profile {
    initial_profile_impl()
}

#[cfg(not(feature = "hydrate"))]
fn initial_profile_impl() -> crate::client::storage::Profile {
    crate::client::storage::Profile::default()
}

#[cfg(feature = "hydrate")]
fn initial_profile_impl() -> crate::client::storage::Profile {
    crate::client::storage::load_profile()
}

fn initial_recent_rooms() -> Vec<crate::client::storage::RecentRoom> {
    initial_recent_rooms_impl()
}

#[cfg(not(feature = "hydrate"))]
fn initial_recent_rooms_impl() -> Vec<crate::client::storage::RecentRoom> {
    Vec::new()
}

#[cfg(feature = "hydrate")]
fn initial_recent_rooms_impl() -> Vec<crate::client::storage::RecentRoom> {
    crate::client::storage::load_recent_rooms()
}

#[cfg(not(feature = "hydrate"))]
fn prune_recent_rooms(_set_recent_rooms: WriteSignal<Vec<crate::client::storage::RecentRoom>>) {}

#[cfg(feature = "hydrate")]
fn prune_recent_rooms(set_recent_rooms: WriteSignal<Vec<crate::client::storage::RecentRoom>>) {
    use leptos::task::spawn_local;

    use crate::client::{rooms_api::check_room, storage::remove_recent_room};

    for room in crate::client::storage::load_recent_rooms() {
        let code = room.code.clone();
        spawn_local(async move {
            if let Some(status) = check_room(&code).await {
                if !status.exists {
                    remove_recent_room(&code);
                    set_recent_rooms.update(|rooms| rooms.retain(|r| r.code != code));
                }
            }
        });
    }
}

#[cfg(not(feature = "hydrate"))]
fn create_room_handler(
    _nick: ReadSignal<String>,
    _color: ReadSignal<String>,
    _room_name: ReadSignal<String>,
    _password: ReadSignal<String>,
    _set_status: WriteSignal<String>,
    _set_submitting: WriteSignal<bool>,
) -> impl Fn(leptos::ev::SubmitEvent) + 'static {
    move |ev: leptos::ev::SubmitEvent| ev.prevent_default()
}

#[cfg(feature = "hydrate")]
fn create_room_handler(
    nick: ReadSignal<String>,
    color: ReadSignal<String>,
    room_name: ReadSignal<String>,
    password: ReadSignal<String>,
    set_status: WriteSignal<String>,
    set_submitting: WriteSignal<bool>,
) -> impl Fn(leptos::ev::SubmitEvent) + 'static {
    use std::cell::RefCell;
    use std::rc::Rc;

    use leptos_router::hooks::use_navigate;

    use crate::client::session::{self, PendingSession};
    use crate::client::socket::WsClient;
    use crate::client::storage::{save_profile, save_recent_room, Profile, RecentRoom};
    use crate::signaling::protocol::{ClientMessage, ServerMessage};

    move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();

        let nick_value = nick.get_untracked().trim().to_string();
        let color_value = color.get_untracked();
        let room_name_value = room_name.get_untracked().trim().to_string();
        let password_value = password.get_untracked();
        if nick_value.is_empty() || room_name_value.is_empty() || password_value.is_empty() {
            set_status.set("Preencha todos os campos.".to_string());
            return;
        }

        set_submitting.set(true);
        set_status.set("Criando sala...".to_string());

        let ws_slot: Rc<RefCell<Option<WsClient>>> = Rc::new(RefCell::new(None));
        let navigate = use_navigate();

        let on_message = {
            let ws_slot = ws_slot.clone();
            let nick_value = nick_value.clone();
            let color_value = color_value.clone();
            move |msg: ServerMessage| {
                if let ServerMessage::Joined { peer_id, room, room_name, members, active_sharers } = msg {
                    save_profile(&Profile { nick: nick_value.clone(), color: color_value.clone() });
                    save_recent_room(RecentRoom { code: room.clone(), name: room_name.clone() });
                    if let Some(ws) = ws_slot.borrow_mut().take() {
                        session::stash(PendingSession {
                            room: room.clone(),
                            room_name,
                            ws,
                            peer_id,
                            members,
                            active_sharers,
                        });
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
                    let color_for_open = color_value.clone();
                    let room_name_for_open = room_name_value.clone();
                    let password_for_open = password_value.clone();
                    move || {
                        if let Some(ws) = ws_slot.borrow().as_ref() {
                            ws.send(&ClientMessage::CreateRoom {
                                nick: nick_for_open.clone(),
                                password: password_for_open.clone(),
                                room_name: room_name_for_open.clone(),
                                color: color_for_open.clone(),
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

> `<For>` na lista de salas recentes é seguro aqui — o `<a href=...>` não captura
> `WsClient`/`RoomConnection`, só uma `String` estática, então não esbarra na
> restrição de `Send + Sync` que força `class:hidden` em outros lugares deste
> projeto.

- [ ] **Step 3: Verificar manualmente no navegador**

Run: `cargo leptos watch`, abra `http://127.0.0.1:3000/`.
Expected: formulário com nick, paleta de cores (clicar numa cor a marca como
selecionada), nome da sala e senha, todos obrigatórios. Ao criar uma sala, navega
pra `/r/<código>`. Reabra `/` — a sala aparece em "Salas recentes" com o nome
certo.

- [ ] **Step 4: Verificar os dois alvos e a suíte de testes**

Run: `cargo check --features ssr --bin screen_share`
Run: `cargo check --features hydrate --target wasm32-unknown-unknown --lib`
Run: `cargo test --features ssr`
Expected: sucesso nos três (a Task 8 é quem vai atualizar `room.rs` pra bater com
o `PendingSession` novo — até lá, `room.rs` não compila; rode esses comandos de
novo só depois da Task 8 se quiser confirmar o build completo agora).

- [ ] **Step 5: Commit**

```bash
git add src/pages/home.rs src/client/session.rs
git commit -m "feat: add room name, color picker, and recent rooms to the home page"
```

---

## Task 8: Página de sala — checagem imediata, nome/cor no portão e cards de membro

Esta task é uma unidade só, embora grande: a checagem imediata de sala e os cards
de membro (avatar, cor) só compilam juntos, porque ambos tocam as mesmas quatro
funções de fiação de conexão (`apply_joined_snapshot`, `build_message_handler`,
`adopt_pending_session`, `setup_room_connection`) — não dá pra fazer uma metade
sem a outra sem deixar o arquivo num estado que não compila no meio do caminho.

**Files:**
- Modify: `src/pages/room.rs`
- Modify: `src/pages/status.rs` (corrigir classificação de "sala não encontrada")

**Interfaces:**
- Consumes: tudo que a Task 7 produz (`PendingSession.room_name`, `client::storage::{load_profile, save_profile, RecentRoom, save_recent_room}` — substituindo `load_nick`/`save_nick`), `client::rooms_api::check_room`, `pages::palette::{palette_ids, color_hex, avatar_letter}`, `protocol::{MemberInfo, MAX_MEMBERS, ClientMessage, ServerMessage}` com os campos novos.
- Produces: `apply_joined_snapshot`, `build_message_handler`, `adopt_pending_session`,
  `setup_room_connection` com as assinaturas finais desta fase (`room_code`/
  `room_name` inclusos, ainda sem `watching`) e `member_cards` — tudo consumido
  pela Task 9 (assistir sob demanda), que ainda vai estender essas mesmas
  assinaturas.

- [ ] **Step 1: Corrigir a classificação de status "sala não encontrada"**

Em `src/pages/status.rs`, a linha `s if s.starts_with("Sessão não encontrada") =>
("error", "NÃO ENCONTRADA")` é sobra do v1 — a mensagem do v2/v3 é "Sala não
encontrada...", que nunca bateu com esse prefixo (bug preexistente: o texto
aparecia sem o estilo de erro). Troque:

```rust
s if s.starts_with("Sessão não encontrada") => ("error", "NÃO ENCONTRADA"),
```

por:

```rust
s if s.starts_with("Sala não encontrada") => ("error", "NÃO ENCONTRADA"),
```

- [ ] **Step 2: Substituir o começo de `src/pages/room.rs`**

Substitua desde `use leptos::prelude::*;` até o fim do componente `RoomPage`
(tudo antes de `fn initial_nick`) por:

```rust
use leptos::prelude::*;
use leptos_router::hooks::use_params_map;

use crate::pages::palette::{color_hex, palette_ids};
use crate::pages::status::status_meta;
use crate::signaling::protocol::MAX_MEMBERS;

#[derive(Clone, PartialEq)]
pub struct RoomMember {
    pub peer_id: String,
    pub nick: String,
    pub color: String,
    pub sharing: bool,
}

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

    let profile = initial_profile();
    let (nick, set_nick) = signal(profile.nick);
    let (color, set_color) = signal(profile.color);
    let (password, set_password) = signal(String::new());
    let (status, set_status) = signal("Informe o nick e a senha da sala.".to_string());
    let (authenticated, set_authenticated) = signal(false);
    let (room_exists, set_room_exists) = signal(None::<bool>);
    let (room_name, set_room_name) = signal(None::<String>);
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
        set_room_name,
        set_members,
        set_my_peer_id,
        connection_errors,
    );

    adopt_pending_session(
        initial_code.clone(),
        conn.clone(),
        set_status,
        set_authenticated,
        set_room_name,
        set_members,
        set_my_peer_id,
        connection_errors,
    );

    start_room_check(initial_code, authenticated, set_room_exists, set_room_name);

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
            join_room(nick_value, color.get_untracked(), password_value);
        }
    };

    let lamp_class = move || {
        let (variant, _) = status_meta(&status.get());
        format!("lamp lamp--{variant}")
    };

    view! {
        <div
            class="panel"
            class:hidden=move || authenticated.get() || room_exists.get().is_some()
        >
            <h1>"Verificando sala..."</h1>
            <p class="status-row__meta">{code}</p>
        </div>
        <div
            class="panel"
            class:hidden=move || authenticated.get() || room_exists.get() != Some(false)
        >
            <h1>"Sala não encontrada"</h1>
            <p class="status-text status-text--error">"Sala não encontrada ou já foi encerrada."</p>
        </div>
        // As seções abaixo ficam sempre montadas e alternam por CSS
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
        <div class="panel" class:hidden=move || authenticated.get() || room_exists.get() != Some(true)>
            <h1>"Entrar na sala"</h1>
            <p class="status-row__meta">
                {move || room_name.get().unwrap_or_default()} " — " {code}
            </p>
            <form on:submit=manual_join.clone()>
                <label class="field">
                    <span class="field__label">"Nick"</span>
                    <input class="field__input" type="text" required prop:value=nick
                        on:input:target=move |ev| set_nick.set(ev.target().value())/>
                </label>
                <div class="field">
                    <span class="field__label">"Sua cor"</span>
                    <div class="color-picker">
                        {palette_ids()
                            .map(|id| {
                                let (border, _) = color_hex(id);
                                view! {
                                    <button
                                        type="button"
                                        class="color-swatch"
                                        class:color-swatch--selected=move || color.get() == id
                                        style=format!("background-color: {border}")
                                        on:click=move |_| set_color.set(id.to_string())
                                    ></button>
                                }
                            })
                            .collect::<Vec<_>>()}
                    </div>
                </div>
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
                <span class="status-row__meta">{move || room_name.get().unwrap_or_default()}</span>
                <span class="status-row__spacer"></span>
            </div>
            <div class="grid">
                {member_cards(members, my_peer_id, is_sharing, local_video_ref, connection_errors)}
            </div>
        </div>
    }
}
```

> O botão de compartilhar/parar do v2 some do cabeçalho por enquanto — a Task 9
> (assistir sob demanda) devolve um botão equivalente ali. `member_cards`, a
> função que gera o conteúdo de `<div class="grid">`, é implementada no Step 4.

- [ ] **Step 3: Trocar `initial_nick`/`save_nick` por perfil, e adicionar a checagem de sala**

Substitua as duas funções `initial_nick` (a `#[cfg(not(feature = "hydrate"))]` e a
`#[cfg(feature = "hydrate")]`) por:

```rust
fn initial_profile() -> crate::client::storage::Profile {
    initial_profile_impl()
}

#[cfg(not(feature = "hydrate"))]
fn initial_profile_impl() -> crate::client::storage::Profile {
    crate::client::storage::Profile::default()
}

#[cfg(feature = "hydrate")]
fn initial_profile_impl() -> crate::client::storage::Profile {
    crate::client::storage::load_profile()
}

#[cfg(not(feature = "hydrate"))]
fn start_room_check(
    _room_code: String,
    _authenticated: ReadSignal<bool>,
    _set_room_exists: WriteSignal<Option<bool>>,
    _set_room_name: WriteSignal<Option<String>>,
) {
}

#[cfg(feature = "hydrate")]
fn start_room_check(
    room_code: String,
    authenticated: ReadSignal<bool>,
    set_room_exists: WriteSignal<Option<bool>>,
    set_room_name: WriteSignal<Option<String>>,
) {
    use leptos::task::spawn_local;

    use crate::client::rooms_api::check_room;

    spawn_local(async move {
        let result = check_room(&room_code).await;
        // Se a sessão pendente da home já autenticou enquanto essa checagem
        // estava em voo, ignora o resultado — já sabemos que a sala existe.
        if authenticated.get_untracked() {
            return;
        }
        match result {
            Some(status) if status.exists => {
                set_room_name.set(status.name);
                set_room_exists.set(Some(true));
            }
            Some(_) => set_room_exists.set(Some(false)),
            None => set_room_exists.set(Some(true)), // rede falhou: não bloqueia, deixa tentar entrar
        }
    });
}
```

- [ ] **Step 4: Implementar a função que constrói os `MAX_MEMBERS` cards**

No final de `src/pages/room.rs`, depois de `start_room_check`, adicione:

```rust
/// Constrói `MAX_MEMBERS` cards estáticos, uma vez só — não uma `<For>`
/// reativa. Cada card vai ganhar botões (assistir, parar de assistir,
/// expandir) que capturam `RoomConnection` (Task 9), e o Leptos 0.8 exige
/// `Send + Sync` de qualquer closure de filho dinâmico usada por `<For>`
/// (mesma restrição documentada no topo do arquivo pra `<Show>`). Construir
/// os cards uma única vez, fora de qualquer closure reativa, e deixar toda a
/// reatividade nos atributos internos (`class:hidden`, `{move || ...}`)
/// evita esse requisito — o mesmo padrão já usado pro portão de autenticação
/// e pelo botão de compartilhar do v2.
///
/// Cada slot `i` mostra o membro atualmente na posição `i` de `members`
/// (não um id fixo) — se alguém sai, os slots depois dele deslizam pra cima.
/// É uma simplificação aceita: a sala não promete posição estável por
/// membro, só mostrar quem está presente agora.
fn member_cards(
    members: ReadSignal<Vec<RoomMember>>,
    my_peer_id: ReadSignal<Option<String>>,
    is_sharing: ReadSignal<bool>,
    local_video_ref: NodeRef<leptos::html::Video>,
    connection_errors: RwSignal<std::collections::HashSet<String>>,
) -> Vec<impl IntoView> {
    (0..MAX_MEMBERS)
        .map(|i| {
            let member_at = move || members.get().get(i).cloned();
            let is_self = move || {
                member_at()
                    .zip(my_peer_id.get())
                    .map(|(m, my_id)| m.peer_id == my_id)
                    .unwrap_or(false)
            };
            // Por enquanto (antes da Task 9) só o próprio card mostra vídeo,
            // exatamente como no v2 — cards de outros membros só têm avatar.
            let showing_video = move || is_self() && is_sharing.get();

            view! {
                <div class="card" class:hidden=move || member_at().is_none()>
                    <div
                        class="card__avatar"
                        class:hidden=showing_video
                        style=move || {
                            let (border, bg) = member_at().map(|m| color_hex(&m.color)).unwrap_or(("#b0b8c1", "#2a2d31"));
                            format!("background-color: {bg}; border-color: {border};")
                        }
                    >
                        <span class="card__avatar-letter">
                            {move || member_at().map(|m| avatar_letter(&m.nick)).unwrap_or_default()}
                        </span>
                    </div>
                    <video
                        node_ref=local_video_ref
                        class:hidden=move || !(is_self() && showing_video())
                        autoplay=true
                        playsinline=true
                        muted=true
                    ></video>
                    <video
                        id=move || member_at().map(|m| format!("video-{}", m.peer_id)).unwrap_or_default()
                        class:hidden=move || !(!is_self() && showing_video())
                        autoplay=true
                        playsinline=true
                    ></video>
                    <div
                        class="card__error"
                        class:hidden=move || {
                            member_at().map(|m| !connection_errors.get().contains(&m.peer_id)).unwrap_or(true)
                        }
                    >
                        "Não foi possível conectar."
                    </div>
                    <div class="card__footer">
                        <span class="card__nick">{move || member_at().map(|m| m.nick).unwrap_or_default()}</span>
                    </div>
                </div>
            }
        })
        .collect::<Vec<_>>()
}
```

> Dois elementos `<video>` por card (um pro seu próprio preview, um pro vídeo de
> quem você assiste) em vez de um só reaproveitado: o `node_ref` do preview
> local precisa ficar fixo num elemento específico (usado por
> `share_toggle_handler` pra setar `srcObject`), e o vídeo de quem você assiste
> é localizado por `id` a partir do `peer_id` (usado pelo `ontrack`). Nunca os
> dois ficam visíveis ao mesmo tempo no mesmo card (um card é seu ou é de
> outra pessoa, nunca as duas coisas), então não há duplicação visual — só
> дois nós DOM reservados, um deles sempre `hidden`.

- [ ] **Step 5: Atualizar `apply_joined_snapshot` e `build_message_handler` pra ler `color`**

Troque a implementação de `apply_joined_snapshot` (ganha `room_code`/`room_name`
como parâmetros novos, e salva em salas recentes) por:

```rust
#[cfg(feature = "hydrate")]
fn apply_joined_snapshot(
    room_code: String,
    room_name: String,
    peer_id: String,
    joined_members: Vec<crate::signaling::protocol::MemberInfo>,
    active_sharers: Vec<String>,
    set_my_peer_id: WriteSignal<Option<String>>,
    set_members: WriteSignal<Vec<RoomMember>>,
    set_room_name: WriteSignal<Option<String>>,
    set_authenticated: WriteSignal<bool>,
    set_status: WriteSignal<String>,
) {
    use std::collections::HashSet;

    use crate::client::storage::{save_recent_room, RecentRoom};

    let sharer_set: HashSet<String> = active_sharers.into_iter().collect();
    let members: Vec<RoomMember> = joined_members
        .into_iter()
        .map(|m| RoomMember { sharing: sharer_set.contains(&m.peer_id), peer_id: m.peer_id, nick: m.nick, color: m.color })
        .collect();
    save_recent_room(RecentRoom { code: room_code, name: room_name.clone() });
    set_my_peer_id.set(Some(peer_id));
    set_members.set(members);
    set_room_name.set(Some(room_name));
    set_authenticated.set(true);
    set_status.set("Conectado.".to_string());
}
```

Troque a assinatura e o braço `Joined` de `build_message_handler`:

```rust
#[cfg(feature = "hydrate")]
fn build_message_handler(
    set_status: WriteSignal<String>,
    set_authenticated: WriteSignal<bool>,
    set_room_name: WriteSignal<Option<String>>,
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
        ServerMessage::Joined { peer_id, room, room_name, members: joined_members, active_sharers } => {
            apply_joined_snapshot(
                room,
                room_name,
                peer_id,
                joined_members,
                active_sharers,
                set_my_peer_id,
                set_members,
                set_room_name,
                set_authenticated,
                set_status,
            );
        }
        ServerMessage::AuthFailed => set_status.set("Senha incorreta.".to_string()),
        ServerMessage::RoomNotFound => set_status.set("Sala não encontrada ou já foi encerrada.".to_string()),
        ServerMessage::RoomFull => set_status.set("Essa sala já está cheia (máximo de 10 pessoas).".to_string()),
        ServerMessage::PeerJoined { peer_id, nick, color } => {
            set_members.update(|members| members.push(RoomMember { peer_id, nick, color, sharing: false }));
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
        // WatchRequested/WatchStopped chegam na Task 9.
        _ => {}
    }
}
```

`adopt_pending_session` e `setup_room_connection` também precisam repassar
`room_name`/`set_room_name`. Atualize as quatro variantes (stub e real de cada
uma):

```rust
#[cfg(not(feature = "hydrate"))]
fn adopt_pending_session(
    _room_code: String,
    _conn: RoomConnection,
    _set_status: WriteSignal<String>,
    _set_authenticated: WriteSignal<bool>,
    _set_room_name: WriteSignal<Option<String>>,
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
    set_room_name: WriteSignal<Option<String>>,
    set_members: WriteSignal<Vec<RoomMember>>,
    set_my_peer_id: WriteSignal<Option<String>>,
    connection_errors: RwSignal<std::collections::HashSet<String>>,
) {
    use crate::client::session;

    let Some(mut session) = session::take(&room_code) else { return };

    let on_message = build_message_handler(set_status, set_authenticated, set_room_name, set_members, set_my_peer_id, conn.clone(), connection_errors);
    session.ws.set_on_message(on_message);
    session.ws.on_close(move || {
        set_status.set("Conexão perdida. Recarregue a página para tentar de novo.".to_string());
    });

    apply_joined_snapshot(
        session.room,
        session.room_name,
        session.peer_id,
        session.members,
        session.active_sharers,
        set_my_peer_id,
        set_members,
        set_room_name,
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
    _set_room_name: WriteSignal<Option<String>>,
    _set_members: WriteSignal<Vec<RoomMember>>,
    _set_my_peer_id: WriteSignal<Option<String>>,
    _connection_errors: RwSignal<std::collections::HashSet<String>>,
) -> impl Fn(String, String, String) + Clone + 'static {
    move |_nick: String, _color: String, _password: String| {}
}

#[cfg(feature = "hydrate")]
fn setup_room_connection(
    room_code: String,
    conn: RoomConnection,
    set_status: WriteSignal<String>,
    set_authenticated: WriteSignal<bool>,
    set_room_name: WriteSignal<Option<String>>,
    set_members: WriteSignal<Vec<RoomMember>>,
    set_my_peer_id: WriteSignal<Option<String>>,
    connection_errors: RwSignal<std::collections::HashSet<String>>,
) -> impl Fn(String, String, String) + Clone + 'static {
    use crate::client::socket::WsClient;
    use crate::client::storage::{save_profile, Profile};
    use crate::signaling::protocol::ClientMessage;

    move |nick: String, color: String, password: String| {
        let conn = conn.clone();
        let room_code = room_code.clone();
        set_status.set("Conectando...".to_string());

        let on_message = build_message_handler(set_status, set_authenticated, set_room_name, set_members, set_my_peer_id, conn.clone(), connection_errors);

        match WsClient::connect("/ws", on_message) {
            Ok(ws) => {
                ws.on_open({
                    let conn = conn.clone();
                    let room_code = room_code.clone();
                    let nick = nick.clone();
                    let color = color.clone();
                    let password = password.clone();
                    move || {
                        if let Some(ws) = conn.ws.borrow().as_ref() {
                            ws.send(&ClientMessage::JoinRoom { room: room_code.clone(), nick: nick.clone(), password: password.clone(), color: color.clone() });
                        }
                    }
                });
                ws.on_close(move || {
                    set_status.set("Conexão perdida. Recarregue a página para tentar de novo.".to_string());
                });
                *conn.ws.borrow_mut() = Some(ws);
                save_profile(&Profile { nick, color });
            }
            Err(_) => set_status.set("Não foi possível conectar ao servidor.".to_string()),
        }
    }
}
```

`join_room` (o closure retornado por `setup_room_connection`) já recebe 3
argumentos desde o Step 2 (`join_room(nick_value, color.get_untracked(),
password_value)`, dentro de `manual_join`) — nenhuma mudança extra necessária
ali.

Remova também `load_nick`/`save_nick` de `src/client/storage.rs` — depois desta
task, nenhum lugar do código chama mais essas duas funções (foram substituídas
por `Profile`/`load_profile`/`save_profile` na Task 7 e neste Step 5).

- [ ] **Step 6: Verificar nos dois alvos e rodar a suíte**

Run: `cargo check --features ssr --bin screen_share`
Run: `cargo check --features hydrate --target wasm32-unknown-unknown --lib`
Run: `cargo test --features ssr`
Expected: sucesso nos três.

- [ ] **Step 7: Verificar manualmente no navegador**

Run: `cargo leptos watch`.
1. Abra `/r/ZZZZZZZZ` (código inexistente) — Expected: "Verificando sala..." por
   um instante, depois "Sala não encontrada", sem nenhum formulário aparecer.
2. Crie uma sala pela home com nick "Ana" e uma cor da paleta — Expected: entra
   direto (sem "Verificando..." nem formulário, igual v2), e o card da Ana
   aparece com avatar circular (inicial "A", cor escolhida) no lugar dela na
   grade.
3. Copie o link dessa sala e abra numa aba nova — Expected: "Verificando
   sala..." rapidamente, depois o formulário com o nome da sala no lugar do
   código antigo, com a paleta de cores pra escolher. Entre com nick "Bia" e
   outra cor — Expected: o card da Bia aparece na grade da Ana (com avatar/cor
   certos) e vice-versa.
4. Compartilhe a tela da Ana — Expected: o card dela mostra o preview local (o
   vídeo, não o avatar); os cards dos outros membros continuam mostrando só
   avatar (nenhum vídeo chega até a Task 9 existir).

- [ ] **Step 8: Commit**

```bash
git add src/pages/room.rs src/pages/status.rs src/client/storage.rs
git commit -m "feat: check room existence immediately and render member cards with avatar/color"
```

---

## Task 9: Assistir sob demanda

**Files:**
- Modify: `src/pages/room.rs`

**Interfaces:**
- Consumes: `ClientMessage::{WatchShare, StopWatching}` / `ServerMessage::{WatchRequested, WatchStopped}` (Task 1), `RoomConnection` (Task 8).
- Produces: comportamento de assistir completo — nenhuma interface nova exposta
  fora do arquivo.

- [ ] **Step 1: Trocar `share_toggle_handler` — não manda mais oferta pra todo mundo**

Substitua a variante `#[cfg(feature = "hydrate")]` de `share_toggle_handler` (a
`#[cfg(not(feature = "hydrate"))]` não muda) — o corpo do `spawn_local` some, já
que abrir conexão de saída deixa de acontecer no clique de compartilhar:

```rust
#[cfg(feature = "hydrate")]
fn share_toggle_handler(
    conn: RoomConnection,
    is_sharing: ReadSignal<bool>,
    set_is_sharing: WriteSignal<bool>,
    set_status: WriteSignal<String>,
    local_video_ref: NodeRef<leptos::html::Video>,
) -> impl Fn(leptos::ev::MouseEvent) + Clone + 'static {
    use leptos::task::spawn_local;
    use wasm_bindgen::JsCast;
    use web_sys::MediaStreamTrack;

    use crate::client::webrtc::capture_display;
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
            // também precisa disparar a mesma limpeza — sem isso, quem
            // estava assistindo fica com a última imagem congelada.
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
        });
    }
}
```

E a variante stub:

```rust
#[cfg(not(feature = "hydrate"))]
fn share_toggle_handler(
    _conn: RoomConnection,
    _is_sharing: ReadSignal<bool>,
    _set_is_sharing: WriteSignal<bool>,
    _set_status: WriteSignal<String>,
    _local_video_ref: NodeRef<leptos::html::Video>,
) -> impl Fn(leptos::ev::MouseEvent) + Clone + 'static {
    move |_| {}
}
```

`stop_sharing` não muda — já fecha tudo que estiver em `conn.outgoing`,
não importa se a conexão foi aberta por fan-out (v2) ou por pedido individual
(v3).

- [ ] **Step 2: Adicionar `WatchRequested`/`WatchStopped` em `build_message_handler`**

No `match msg` de `build_message_handler`, logo antes do braço final `_ => {}`
(que agora só cobre o resto de mensagens irrelevantes pro cliente), adicione:

```rust
ServerMessage::WatchRequested { from } => {
    let conn = conn.clone();
    spawn_local(async move {
        use web_sys::{MediaStreamTrack, RtcPeerConnectionIceEvent};

        use crate::client::webrtc::create_offer;
        use crate::signaling::protocol::ClientMessage;

        let Ok(pc) = new_peer_connection() else { return };
        conn.outgoing.borrow_mut().insert(from.clone(), pc.clone());
        connection_errors.update(|errors| { errors.remove(&from); });

        if let Some(stream) = conn.local_stream.borrow().as_ref() {
            for track in stream.get_tracks().iter() {
                let track: MediaStreamTrack = track.unchecked_into();
                pc.add_track_0(&track, stream);
            }
        }

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

        let failed_viewer_id = from.clone();
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
                ws.send(&ClientMessage::Offer { to: from, sdp });
            }
        }
    });
}
ServerMessage::WatchStopped { from } => {
    if let Some(pc) = conn.outgoing.borrow_mut().remove(&from) {
        pc.close();
    }
}
```

> Note a checagem `if let Some(stream) = conn.local_stream.borrow().as_ref()` —
> se `WatchRequested` chegar depois que a pessoa já parou de compartilhar
> (corrida entre alguém clicar assistir e você parar ao mesmo tempo), o
> `local_stream` já foi limpo por `stop_sharing`, então a oferta simplesmente
> não leva nenhuma track — o espectador recebe uma conexão sem vídeo. Isso é
> aceitável (raro, e a UI do espectador já mostra "conectando" até a
> primeira imagem chegar); adicionar uma checagem explícita de
> "ainda estou compartilhando?" abortando a oferta é um refinamento possível
> mas não necessário pro comportamento correto.

`new_peer_connection`/`accept_answer`/`add_ice_candidate` já estão importados no
topo de `build_message_handler` desde a Task 8 — nenhum import novo necessário
ali, só dentro deste novo bloco (`create_offer`, que não era usado nesta função
antes).

- [ ] **Step 3: Adicionar os botões de assistir/parar aos cards de outros membros**

Em `member_cards` (Task 8), a função ganha parâmetros novos e os botões de
assistir. Substitua a assinatura e o corpo:

```rust
fn member_cards(
    conn: RoomConnection,
    members: ReadSignal<Vec<RoomMember>>,
    my_peer_id: ReadSignal<Option<String>>,
    is_sharing: ReadSignal<bool>,
    watching: RwSignal<std::collections::HashSet<String>>,
    local_video_ref: NodeRef<leptos::html::Video>,
    connection_errors: RwSignal<std::collections::HashSet<String>>,
) -> Vec<impl IntoView> {
    (0..MAX_MEMBERS)
        .map(|i| {
            let member_at = move || members.get().get(i).cloned();
            let is_self = move || {
                member_at()
                    .zip(my_peer_id.get())
                    .map(|(m, my_id)| m.peer_id == my_id)
                    .unwrap_or(false)
            };
            let is_watching_this = move || {
                member_at().map(|m| watching.get().contains(&m.peer_id)).unwrap_or(false)
            };
            let showing_video = move || (is_self() && is_sharing.get()) || (!is_self() && is_watching_this());
            let can_watch = move || {
                member_at().map(|m| m.sharing).unwrap_or(false) && !is_self() && !is_watching_this()
            };

            let watch = watch_click_handler(conn.clone(), members, watching, i);
            let stop_watch = stop_watching_click_handler(conn.clone(), members, watching, i);

            view! {
                <div class="card" class:hidden=move || member_at().is_none()>
                    <div
                        class="card__avatar"
                        class:hidden=showing_video
                        style=move || {
                            let (border, bg) = member_at().map(|m| color_hex(&m.color)).unwrap_or(("#b0b8c1", "#2a2d31"));
                            format!("background-color: {bg}; border-color: {border};")
                        }
                    >
                        <span class="card__avatar-letter">
                            {move || member_at().map(|m| avatar_letter(&m.nick)).unwrap_or_default()}
                        </span>
                    </div>
                    <video
                        node_ref=local_video_ref
                        class:hidden=move || !(is_self() && showing_video())
                        autoplay=true
                        playsinline=true
                        muted=true
                    ></video>
                    <video
                        id=move || member_at().map(|m| format!("video-{}", m.peer_id)).unwrap_or_default()
                        class:hidden=move || !(!is_self() && showing_video())
                        autoplay=true
                        playsinline=true
                    ></video>
                    <div
                        class="card__error"
                        class:hidden=move || {
                            member_at().map(|m| !connection_errors.get().contains(&m.peer_id)).unwrap_or(true)
                        }
                    >
                        "Não foi possível conectar."
                    </div>
                    <div class="card__footer">
                        <span class="card__nick">{move || member_at().map(|m| m.nick).unwrap_or_default()}</span>
                        <div class="card__actions">
                            <button class="btn--ghost" class:hidden=move || !can_watch() on:click=watch.clone()>
                                "Assistir compartilhamento"
                            </button>
                            <button class="btn--ghost" class:hidden=move || !is_watching_this() on:click=stop_watch.clone()>
                                "Parar de assistir"
                            </button>
                        </div>
                    </div>
                </div>
            }
        })
        .collect::<Vec<_>>()
}

#[cfg(not(feature = "hydrate"))]
fn watch_click_handler(
    _conn: RoomConnection,
    _members: ReadSignal<Vec<RoomMember>>,
    _watching: RwSignal<std::collections::HashSet<String>>,
    _slot: usize,
) -> impl Fn(leptos::ev::MouseEvent) + Clone + 'static {
    move |_| {}
}

#[cfg(feature = "hydrate")]
fn watch_click_handler(
    conn: RoomConnection,
    members: ReadSignal<Vec<RoomMember>>,
    watching: RwSignal<std::collections::HashSet<String>>,
    slot: usize,
) -> impl Fn(leptos::ev::MouseEvent) + Clone + 'static {
    use crate::signaling::protocol::ClientMessage;

    move |_| {
        let Some(member) = members.get_untracked().get(slot).cloned() else { return };
        watching.update(|w| { w.insert(member.peer_id.clone()); });
        if let Some(ws) = conn.ws.borrow().as_ref() {
            ws.send(&ClientMessage::WatchShare { sharer_id: member.peer_id });
        }
    }
}

#[cfg(not(feature = "hydrate"))]
fn stop_watching_click_handler(
    _conn: RoomConnection,
    _members: ReadSignal<Vec<RoomMember>>,
    _watching: RwSignal<std::collections::HashSet<String>>,
    _slot: usize,
) -> impl Fn(leptos::ev::MouseEvent) + Clone + 'static {
    move |_| {}
}

#[cfg(feature = "hydrate")]
fn stop_watching_click_handler(
    conn: RoomConnection,
    members: ReadSignal<Vec<RoomMember>>,
    watching: RwSignal<std::collections::HashSet<String>>,
    slot: usize,
) -> impl Fn(leptos::ev::MouseEvent) + Clone + 'static {
    use crate::signaling::protocol::ClientMessage;

    move |_| {
        let Some(member) = members.get_untracked().get(slot).cloned() else { return };
        watching.update(|w| { w.remove(&member.peer_id); });
        if let Some(pc) = conn.incoming.borrow_mut().remove(&member.peer_id) {
            pc.close();
        }
        if let Some(ws) = conn.ws.borrow().as_ref() {
            ws.send(&ClientMessage::StopWatching { sharer_id: member.peer_id });
        }
    }
}
```

`PeerStoppedSharing`, em `build_message_handler` (Task 8), ganha uma linha a
mais — precisa tirar o peer de `watching` também, não só fechar a conexão:

```rust
ServerMessage::PeerStoppedSharing { peer_id } => {
    set_members.update(|members| {
        if let Some(m) = members.iter_mut().find(|m| m.peer_id == peer_id) {
            m.sharing = false;
        }
    });
    watching.update(|w| { w.remove(&peer_id); });
    if let Some(pc) = conn.incoming.borrow_mut().remove(&peer_id) {
        pc.close();
    }
}
```

Isso exige adicionar `watching: RwSignal<std::collections::HashSet<String>>` como
mais um parâmetro de `build_message_handler`, logo antes de `connection_errors`.
Troque a linha de assinatura (definida na Task 8):

```rust
fn build_message_handler(
    set_status: WriteSignal<String>,
    set_authenticated: WriteSignal<bool>,
    set_room_name: WriteSignal<Option<String>>,
    set_members: WriteSignal<Vec<RoomMember>>,
    set_my_peer_id: WriteSignal<Option<String>>,
    conn: RoomConnection,
    watching: RwSignal<std::collections::HashSet<String>>,
    connection_errors: RwSignal<std::collections::HashSet<String>>,
) -> impl Fn(crate::signaling::protocol::ServerMessage) + 'static {
```

Repasse `watching` nas quatro funções que chamam `build_message_handler` — a
mesma posição, logo antes de `connection_errors`, em `adopt_pending_session` e
`setup_room_connection` (stub e real de cada uma, definidas na Task 8). As
quatro assinaturas ficam:

```rust
#[cfg(not(feature = "hydrate"))]
fn adopt_pending_session(
    _room_code: String,
    _conn: RoomConnection,
    _set_status: WriteSignal<String>,
    _set_authenticated: WriteSignal<bool>,
    _set_room_name: WriteSignal<Option<String>>,
    _set_members: WriteSignal<Vec<RoomMember>>,
    _set_my_peer_id: WriteSignal<Option<String>>,
    _watching: RwSignal<std::collections::HashSet<String>>,
    _connection_errors: RwSignal<std::collections::HashSet<String>>,
) {
}

#[cfg(feature = "hydrate")]
fn adopt_pending_session(
    room_code: String,
    conn: RoomConnection,
    set_status: WriteSignal<String>,
    set_authenticated: WriteSignal<bool>,
    set_room_name: WriteSignal<Option<String>>,
    set_members: WriteSignal<Vec<RoomMember>>,
    set_my_peer_id: WriteSignal<Option<String>>,
    watching: RwSignal<std::collections::HashSet<String>>,
    connection_errors: RwSignal<std::collections::HashSet<String>>,
) {
    use crate::client::session;

    let Some(mut session) = session::take(&room_code) else { return };

    let on_message = build_message_handler(set_status, set_authenticated, set_room_name, set_members, set_my_peer_id, conn.clone(), watching, connection_errors);
    session.ws.set_on_message(on_message);
    session.ws.on_close(move || {
        set_status.set("Conexão perdida. Recarregue a página para tentar de novo.".to_string());
    });

    apply_joined_snapshot(
        session.room,
        session.room_name,
        session.peer_id,
        session.members,
        session.active_sharers,
        set_my_peer_id,
        set_members,
        set_room_name,
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
    _set_room_name: WriteSignal<Option<String>>,
    _set_members: WriteSignal<Vec<RoomMember>>,
    _set_my_peer_id: WriteSignal<Option<String>>,
    _watching: RwSignal<std::collections::HashSet<String>>,
    _connection_errors: RwSignal<std::collections::HashSet<String>>,
) -> impl Fn(String, String, String) + Clone + 'static {
    move |_nick: String, _color: String, _password: String| {}
}

#[cfg(feature = "hydrate")]
fn setup_room_connection(
    room_code: String,
    conn: RoomConnection,
    set_status: WriteSignal<String>,
    set_authenticated: WriteSignal<bool>,
    set_room_name: WriteSignal<Option<String>>,
    set_members: WriteSignal<Vec<RoomMember>>,
    set_my_peer_id: WriteSignal<Option<String>>,
    watching: RwSignal<std::collections::HashSet<String>>,
    connection_errors: RwSignal<std::collections::HashSet<String>>,
) -> impl Fn(String, String, String) + Clone + 'static {
    use crate::client::socket::WsClient;
    use crate::client::storage::{save_profile, Profile};
    use crate::signaling::protocol::ClientMessage;

    move |nick: String, color: String, password: String| {
        let conn = conn.clone();
        let room_code = room_code.clone();
        set_status.set("Conectando...".to_string());

        let on_message = build_message_handler(set_status, set_authenticated, set_room_name, set_members, set_my_peer_id, conn.clone(), watching, connection_errors);

        match WsClient::connect("/ws", on_message) {
            Ok(ws) => {
                ws.on_open({
                    let conn = conn.clone();
                    let room_code = room_code.clone();
                    let nick = nick.clone();
                    let color = color.clone();
                    let password = password.clone();
                    move || {
                        if let Some(ws) = conn.ws.borrow().as_ref() {
                            ws.send(&ClientMessage::JoinRoom { room: room_code.clone(), nick: nick.clone(), password: password.clone(), color: color.clone() });
                        }
                    }
                });
                ws.on_close(move || {
                    set_status.set("Conexão perdida. Recarregue a página para tentar de novo.".to_string());
                });
                *conn.ws.borrow_mut() = Some(ws);
                save_profile(&Profile { nick, color });
            }
            Err(_) => set_status.set("Não foi possível conectar ao servidor.".to_string()),
        }
    }
}
```

- [ ] **Step 4: Atualizar o corpo de `RoomPage` — sinal `watching` e chamadas atualizadas**

No componente `RoomPage`, logo depois de `let connection_errors = ...`, adicione:

```rust
let watching = RwSignal::new(std::collections::HashSet::<String>::new());
```

E troque as chamadas de `setup_room_connection`, `adopt_pending_session` e
`member_cards` pra incluir `watching`:

```rust
let join_room = setup_room_connection(
    initial_code.clone(),
    conn.clone(),
    set_status,
    set_authenticated,
    set_room_name,
    set_members,
    set_my_peer_id,
    watching,
    connection_errors,
);

adopt_pending_session(
    initial_code.clone(),
    conn.clone(),
    set_status,
    set_authenticated,
    set_room_name,
    set_members,
    set_my_peer_id,
    watching,
    connection_errors,
);
```

E o botão de compartilhar volta pro cabeçalho (sem o botão de assistir — esse já
vive dentro de cada card agora), e a chamada de `member_cards` ganha `conn`/
`watching`:

```rust
let toggle_share = share_toggle_handler(conn.clone(), is_sharing, set_is_sharing, set_status, local_video_ref);
```

(chame isso logo depois de `manual_join`, como no v2), e no `view!`, dentro de
`<div class="stage-header">`:

```rust
<div class="stage-header">
    <span class=lamp_class></span>
    <span class="status-row__meta">{move || room_name.get().unwrap_or_default()}</span>
    <span class="status-row__spacer"></span>
    <button
        class=move || if is_sharing.get() { "btn btn--danger" } else { "btn btn--primary" }
        class:hidden=move || !can_share
        on:click=toggle_share.clone()
    >
        {move || if is_sharing.get() { "Parar de compartilhar" } else { "Compartilhar minha tela" }}
    </button>
    <span class="status-text status-text--error" class:hidden=move || can_share>
        "Seu navegador não suporta compartilhar tela — você ainda pode assistir."
    </span>
</div>
<div class="grid">
    {member_cards(conn, members, my_peer_id, is_sharing, watching, local_video_ref, connection_errors)}
</div>
```

- [ ] **Step 5: Verificar nos dois alvos e rodar a suíte**

Run: `cargo check --features ssr --bin screen_share`
Run: `cargo check --features hydrate --target wasm32-unknown-unknown --lib`
Run: `cargo test --features ssr`
Expected: sucesso nos três.

- [ ] **Step 6: Verificar manualmente no navegador (duas abas)**

Run: `cargo leptos watch`.
1. Aba 1 (Ana) cria a sala; aba 2 (Bia) entra. Aba 1 clica "Compartilhar minha
   tela" — Expected: aba 1 mostra o próprio preview; o card da Ana na aba 2
   continua mostrando só o avatar, com um botão "Assistir compartilhamento"
   novo.
2. Aba 2 clica "Assistir compartilhamento" no card da Ana — Expected: o vídeo
   da Ana aparece no card dela na aba 2 em poucos segundos, e o botão vira
   "Parar de assistir".
3. Abra uma terceira aba (Caio), entre na sala, **não** clique assistir —
   Expected: o card da Ana continua mostrando só avatar pro Caio, mesmo com a
   Bia já assistindo.
4. Clique "Parar de assistir" na aba da Bia — Expected: volta a mostrar avatar
   pra ela; a transmissão da Ana continua ativa (só a conexão da Bia fechou).

- [ ] **Step 7: Commit**

```bash
git add src/pages/room.rs
git commit -m "feat: make watching a screen share opt-in per person"
```

---

## Task 10: Expandir/encolher e esconder o próprio preview

**Files:**
- Modify: `src/pages/room.rs`

**Interfaces:**
- Consumes: `member_cards`, `RoomMember` (Task 9).
- Produces: nenhuma interface nova fora do arquivo — estado 100% local de UI.

- [ ] **Step 1: Adicionar os sinais de estado local em `RoomPage`**

Logo depois de `let watching = ...` (Task 9), adicione:

```rust
let expanded = RwSignal::new(None::<String>);
let own_preview_hidden = RwSignal::new(false);
```

E repasse os dois pra `member_cards`:

```rust
<div class="grid" class:grid--focused=move || expanded.get().is_some()>
    {member_cards(conn, members, my_peer_id, is_sharing, watching, expanded, own_preview_hidden, local_video_ref, connection_errors)}
</div>
```

- [ ] **Step 2: Adicionar os botões de expandir/encolher e esconder preview em `member_cards`**

Troque a assinatura de `member_cards` (definida na Task 9) por:

```rust
fn member_cards(
    conn: RoomConnection,
    members: ReadSignal<Vec<RoomMember>>,
    my_peer_id: ReadSignal<Option<String>>,
    is_sharing: ReadSignal<bool>,
    watching: RwSignal<std::collections::HashSet<String>>,
    expanded: RwSignal<Option<String>>,
    own_preview_hidden: RwSignal<bool>,
    local_video_ref: NodeRef<leptos::html::Video>,
    connection_errors: RwSignal<std::collections::HashSet<String>>,
) -> Vec<impl IntoView> {
```

(só a linha de assinatura muda — o corpo continua o mesmo até o ponto descrito
abaixo). Dentro do `.map(|i| ...)`, depois de `let can_watch = ...`, adicione:

```rust
let is_expanded = move || {
    member_at().map(|m| expanded.get().as_deref() == Some(m.peer_id.as_str())).unwrap_or(false)
};
let own_preview_visible = move || is_self() && is_sharing.get() && !own_preview_hidden.get();
// Com a Task 10, o vídeo do próprio card só aparece se não estiver
// escondido — troca a definição de `showing_video` da Task 9.
let showing_video = move || own_preview_visible() || (!is_self() && is_watching_this());
let can_expand = move || showing_video();

let expand_click = {
    move |_: leptos::ev::MouseEvent| {
        if let Some(member) = member_at() {
            expanded.set(Some(member.peer_id));
        }
    }
};
let shrink_click = move |_: leptos::ev::MouseEvent| expanded.set(None);
let toggle_preview_click = move |_: leptos::ev::MouseEvent| own_preview_hidden.update(|hidden| *hidden = !*hidden);
```

E no `<div class="card__actions">`, adicione três botões (mantendo os de
assistir/parar já existentes):

```rust
<div class="card__actions">
    <button class="btn--ghost" class:hidden=move || !can_watch() on:click=watch.clone()>
        "Assistir compartilhamento"
    </button>
    <button class="btn--ghost" class:hidden=move || !is_watching_this() on:click=stop_watch.clone()>
        "Parar de assistir"
    </button>
    <button class="btn--ghost" class:hidden=move || !(is_self() && is_sharing.get()) on:click=toggle_preview_click>
        {move || if own_preview_hidden.get() { "Mostrar preview" } else { "Esconder preview" }}
    </button>
    <button class="btn--ghost" class:hidden=move || !can_expand() || is_expanded() on:click=expand_click>
        "Expandir"
    </button>
    <button class="btn--ghost" class:hidden=move || !is_expanded() on:click=shrink_click>
        "Encolher"
    </button>
</div>
```

> O botão de esconder/mostrar preview só aparece no seu próprio card, e só
> enquanto você está compartilhando (`is_self() && is_sharing.get()`).

Adicione também `class:card--focus=is_expanded` na `<div class="card" ...>`
raiz de cada card, e o elemento de avatar volta a usar a `showing_video`
redefinida nesta task (nenhuma mudança de código adicional ali — ela já lê a
variável local `showing_video`, que agora reflete `own_preview_hidden`
automaticamente).

- [ ] **Step 3: Verificar nos dois alvos**

Run: `cargo check --features ssr --bin screen_share`
Run: `cargo check --features hydrate --target wasm32-unknown-unknown --lib`
Expected: sucesso.

- [ ] **Step 4: Verificar manualmente no navegador**

Run: `cargo leptos watch`. Compartilhe a tela — Expected: botão "Esconder
preview" aparece só no seu card; clicar nele troca o vídeo pelo avatar sem parar
o compartilhamento de verdade (quem estiver assistindo continua vendo
normalmente). Assista alguém e clique "Expandir" — Expected: o card dela ocupa a
maior parte da grade, os outros cards encolhem; "Encolher" volta ao normal.

- [ ] **Step 5: Commit**

```bash
git add src/pages/room.rs
git commit -m "feat: add expand/collapse and hide-own-preview to member cards"
```

---

## Task 11: Botão de sair da sala

**Files:**
- Modify: `src/pages/room.rs`

**Interfaces:**
- Consumes: `RoomConnection` (Task 8).

- [ ] **Step 1: Implementar o handler de sair**

No final de `src/pages/room.rs`, adicione:

```rust
#[cfg(not(feature = "hydrate"))]
fn leave_room_handler(_conn: RoomConnection) -> impl Fn(leptos::ev::MouseEvent) + Clone + 'static {
    move |_| {}
}

#[cfg(feature = "hydrate")]
fn leave_room_handler(conn: RoomConnection) -> impl Fn(leptos::ev::MouseEvent) + Clone + 'static {
    use leptos_router::hooks::use_navigate;

    move |_| {
        if let Some(ws) = conn.ws.borrow().as_ref() {
            ws.close();
        }
        let navigate = use_navigate();
        navigate("/", Default::default());
    }
}
```

> Fechar o WebSocket já é suficiente — o servidor detecta a desconexão e chama
> `Registry::leave_room` automaticamente (mesmo caminho que já trata fechar a
> aba ou perder a conexão, desde o v2). Não precisa de nenhuma mensagem de
> protocolo nova.

- [ ] **Step 2: Adicionar o botão no cabeçalho da sala**

Logo depois de `let toggle_share = ...` no corpo de `RoomPage`, adicione:

```rust
let leave_room = leave_room_handler(conn.clone());
```

E no `<div class="stage-header">`, adicione o botão como o último filho:

```rust
<div class="stage-header">
    <span class=lamp_class></span>
    <span class="status-row__meta">{move || room_name.get().unwrap_or_default()}</span>
    <span class="status-row__spacer"></span>
    <button
        class=move || if is_sharing.get() { "btn btn--danger" } else { "btn btn--primary" }
        class:hidden=move || !can_share
        on:click=toggle_share.clone()
    >
        {move || if is_sharing.get() { "Parar de compartilhar" } else { "Compartilhar minha tela" }}
    </button>
    <span class="status-text status-text--error" class:hidden=move || can_share>
        "Seu navegador não suporta compartilhar tela — você ainda pode assistir."
    </span>
    <button class="btn--ghost" on:click=leave_room>"Sair da sala"</button>
</div>
```

- [ ] **Step 3: Verificar nos dois alvos**

Run: `cargo check --features ssr --bin screen_share`
Run: `cargo check --features hydrate --target wasm32-unknown-unknown --lib`
Expected: sucesso.

- [ ] **Step 4: Verificar manualmente no navegador**

Run: `cargo leptos watch`. Entre numa sala com duas abas; clique "Sair da sala"
numa delas — Expected: volta pra home; a outra aba continua na sala normalmente
(a sala não morre com uma saída, a menos que fosse a última pessoa).

- [ ] **Step 5: Commit**

```bash
git add src/pages/room.rs
git commit -m "feat: add an explicit leave-room button"
```

---

## Task 12: Estilos — avatar, cores de card, paleta, salas recentes, grade em foco

**Files:**
- Modify: `style/main.css`

**Interfaces:** nenhuma (CSS puro).

- [ ] **Step 1: Remover as classes do v2 que este plano deixa sem uso**

Em `style/main.css`, remova o bloco `/* --- sala: cabeçalho + grade de
transmissões --- */` inteiro — ou seja, as regras `.room-page`,
`.room-page .stage-header`, `.room-page .stage-header .btn`, `.grid`, `.tile`,
`.tile video`, `.tile__label`, `.tile--self` e `.tile__error` — exceto `.grid`,
que continua existindo (só o conteúdo dela mudou, de `.tile`s pra `.card`s).
Mantenha `.grid` com as regras que já tinha.

- [ ] **Step 2: Adicionar os estilos novos**

No final de `style/main.css`, adicione:

```css
/* --- sala: cabeçalho + grade de membros (v3) --- */

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

.card {
  position: relative;
  border-radius: 0.6rem;
  overflow: hidden;
  aspect-ratio: 16 / 9;
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--surface-2);
  border: 2px solid var(--border);
}

.card video {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  object-fit: contain;
  background: #000;
}

.card__avatar {
  width: 4rem;
  height: 4rem;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  border: 2px solid;
}

.card__avatar-letter {
  font-size: 1.6rem;
  font-weight: 700;
  color: #fff;
}

.card__error {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  text-align: center;
  padding: 1rem;
  color: var(--error);
  font-size: 0.85rem;
  background: #111;
}

.card__footer {
  position: absolute;
  left: 0;
  right: 0;
  bottom: 0;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.5rem;
  padding: 0.4rem 0.6rem;
  background: rgba(0, 0, 0, 0.55);
}

.card__nick {
  color: var(--text);
  font-size: 0.82rem;
  font-weight: 600;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.card__actions {
  display: flex;
  gap: 0.35rem;
  flex-wrap: wrap;
  justify-content: flex-end;
}

.card__actions .btn--ghost {
  padding: 0.3rem 0.55rem;
  font-size: 0.72rem;
}

/* Aproximação puramente CSS de "foco + tirinha", sem duplicar nenhum
   elemento (o <video> de cada card é único, precisa continuar sendo o
   mesmo nó DOM pro ontrack/srcObject continuarem funcionando). O card em
   foco recebe posicionamento explícito de grid; os demais ficam em
   auto-placement, o que naturalmente os organiza ao redor do card grande. */
.grid--focused {
  grid-template-columns: repeat(auto-fill, minmax(6rem, 1fr));
  grid-auto-rows: 4.5rem;
}

.grid--focused .card:not(.card--focus) {
  aspect-ratio: auto;
}

.card--focus {
  grid-column: 1 / -1;
  grid-row: span 5;
}

/* --- paleta de cores --- */

.color-picker {
  display: flex;
  flex-wrap: wrap;
  gap: 0.5rem;
}

.color-swatch {
  appearance: none;
  width: 2rem;
  height: 2rem;
  border-radius: 50%;
  border: 2px solid transparent;
  cursor: pointer;
  padding: 0;
}

.color-swatch--selected {
  border-color: var(--text);
}

/* --- salas recentes --- */

.recent-rooms {
  margin-top: 1.5rem;
  padding-top: 1.25rem;
  border-top: 1px solid var(--border);
}

.recent-room {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.5rem;
  padding: 0.6rem 0.75rem;
  border-radius: 0.6rem;
  background: var(--surface-2);
  border: 1px solid var(--border);
  color: var(--text);
  text-decoration: none;
  margin-bottom: 0.5rem;
}

.recent-room:hover {
  border-color: var(--accent-strong);
}

.recent-room__name {
  font-weight: 600;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.recent-room__code {
  font-family: var(--font-mono);
  font-size: 0.75rem;
  color: var(--text-dim);
  flex-shrink: 0;
}
```

- [ ] **Step 3: Verificar manualmente no navegador**

Run: `cargo leptos watch`. Revise: paleta de cores nos formulários de criar/
entrar, avatares circulares nos cards com a cor certa, lista de salas recentes
na home, e o modo expandido (card grande + os outros pequenos ao redor) numa
sala com pelo menos 3 membros.

- [ ] **Step 4: Commit**

```bash
git add style/main.css
git commit -m "style: add avatar, card color, palette, recent-rooms, and focused-grid styles"
```

---

## Task 13: Documentação — `CLAUDE.md` e `README.md`

**Files:**
- Modify: `CLAUDE.md`
- Modify: `README.md`

**Interfaces:** nenhuma (documentação).

- [ ] **Step 1: Atualizar `CLAUDE.md`**

Na seção "## What this project is", depois da frase sobre não ter áudio ainda,
adicione uma frase sobre a identidade visual e o modelo de assistir sob demanda:
cada membro escolhe um nick e uma cor (avatar com a inicial do nick sobre a cor
escolhida); ver a tela de alguém é uma ação explícita — compartilhar não manda
vídeo pra ninguém automaticamente, cada pessoa entra na transmissão de quem
quiser assistir, e pode sair dela a qualquer momento sem afetar outros
espectadores da mesma pessoa.

Na seção "### Room lifecycle", atualize o limite de "8 membros" pra "10
membros" onde for mencionado (se não estiver mencionado literalmente, adicione
uma frase citando o limite de 10).

Adicione uma subseção nova, "### Descoberta de salas", depois de "### Room
lifecycle": cada navegador lembra localmente (via `localStorage`, nunca no
servidor) das salas que aquela pessoa criou ou entrou — código, nome, sem
senha — e mostra isso como "salas recentes" na home; abrir um link de sala
consulta um endpoint HTTP simples (`GET /api/rooms/:code`, fora do protocolo de
WebSocket) pra confirmar que ela ainda existe antes de pedir nick/senha.

- [ ] **Step 2: Atualizar `README.md`**

No parágrafo de abertura, atualize "até 8 pessoas" pra "até 10 pessoas" (se o
número estiver escrito ali; adicione se não estiver) e mencione a identidade
visual (nick + cor) e o modelo de assistir sob demanda.

Na seção "## Checklist de teste manual (fluxo completo)", adicione ao final
(depois do passo que confirma "Sala não encontrada ou já foi encerrada"): um
passo confirmando a checagem imediata (abrir um link de sala inexistente mostra
o aviso antes de qualquer formulário aparecer), um passo confirmando que a
sala aparece em "salas recentes" da home depois de criada/entrada, e um passo
confirmando o modelo de assistir sob demanda (compartilhar não faz o vídeo
aparecer sozinho pra quem não clicou "Assistir compartilhamento").

- [ ] **Step 3: Commit**

```bash
git add CLAUDE.md README.md
git commit -m "docs: update CLAUDE.md and README.md for the Discord-style room model"
```
