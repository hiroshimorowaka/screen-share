# Compartilhamento de Tela P2P — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Um site (sem app instalável) onde uma pessoa compartilha a tela via WebRTC para 2–5 amigos que assistem direto no navegador, usando um servidor Rust único para servir a página e fazer a sinalização WebRTC.

**Architecture:** Um binário Rust (Leptos SSR sobre Axum) serve a aplicação Leptos compilada para WASM e expõe um endpoint WebSocket `/ws` só para sinalização (troca de offer/answer/ICE candidates em JSON). O vídeo nunca passa pelo servidor: quem compartilha abre uma `RTCPeerConnection` por espectador (fan-out P2P direto). Estado de salas é só em memória (sem banco de dados).

**Tech Stack:** Rust, Leptos (SSR + hydração para WASM) via `cargo-leptos`, `leptos_router`, Axum, Tokio, `serde`/`serde_json` para o protocolo de sinalização, `web-sys`/`wasm-bindgen`/`wasm-bindgen-futures` para `getDisplayMedia` e `RTCPeerConnection` no navegador, `uuid` para IDs de peer, `tokio-tungstenite` como dependência de teste para os testes de integração do WebSocket.

## Global Constraints

- 1 pessoa compartilha para um grupo de 2 a 5 espectadores por sessão (fan-out P2P direto do compartilhador, sem SFU).
- Sem contas/login, sem persistência entre sessões (estado só em memória), sem áudio, sem chat, sem gravação.
- NAT traversal: só STUN público (`stun:stun.l.google.com:19302`) na v1; sem TURN.
- Tudo roda no navegador (Windows e Linux) — nenhum app nativo instalável.
- Deploy como um único binário Rust (Leptos SSR + Axum) servindo a página e o WebSocket de sinalização no mesmo processo.
- HTTPS é obrigatório em produção para `getDisplayMedia` e WebSocket seguro (exceto em `localhost`, onde o navegador libera sobre HTTP).

**Nota sobre TDD nesta plan:** as Tasks 2–4 (protocolo e sinalização, puro Rust) seguem o ciclo clássico teste-primeiro. As Tasks 5–9 (frontend Leptos + WebRTC, que só existem dentro de um navegador real) não são unit-testáveis — cada uma delas troca "escreva o teste que falha" por um passo de verificação manual explícito no navegador, conforme já definido na seção de Testes da spec aprovada.

**Nota sobre risco de API:** as chamadas a `web-sys` para WebRTC (Tasks 6–7) usam a convenção de setter `set_<campo>` das dicionários do `web-sys` mais recente. Se o `cargo build` apontar um método inexistente (ex.: `set_sdp` vs `sdp`), isso é só uma mudança de nome entre versões do crate — ajuste conforme o erro do compilador indicar; a lógica ao redor não muda.

---

## Task 1: Scaffold do projeto (cargo-leptos)

**Files:**
- Create: projeto inteiro via `cargo leptos new`
- Modify: `src/app.rs` (conteúdo padrão do template trocado por um placeholder)

**Interfaces:**
- Produces: projeto Rust rodável via `cargo leptos watch`, servindo em `http://127.0.0.1:3000`, com `App` component em `src/app.rs` exportado como `pub fn App() -> impl IntoView`.

- [ ] **Step 1: Instalar pré-requisitos, se ainda não houver**

```bash
rustup target add wasm32-unknown-unknown
cargo install cargo-leptos
```

- [ ] **Step 2: Gerar o projeto a partir do template oficial start-axum**

```bash
cargo leptos new --git https://github.com/leptos-rs/start-axum screen_share
cd screen_share
```

- [ ] **Step 3: Verificar que o template roda**

Run: `cargo leptos watch`
Expected: servidor sobe em `http://127.0.0.1:3000` sem erro; abrir a URL no navegador mostra a página padrão do template (contador de exemplo). Pare o processo (Ctrl+C) depois de confirmar.

- [ ] **Step 4: Substituir o conteúdo padrão por um placeholder do nosso app**

Em `src/app.rs`, troque o corpo do componente `App` (que no template traz o contador de exemplo) por:

```rust
use leptos::prelude::*;
use leptos_meta::{provide_meta_context, MetaTags, Stylesheet, Title};
use leptos_router::{
    components::{Route, Router, Routes},
    path,
};

pub fn shell(options: leptos::config::LeptosOptions) -> impl IntoView {
    view! {
        <!DOCTYPE html>
        <html lang="pt-BR">
            <head>
                <meta charset="utf-8"/>
                <meta name="viewport" content="width=device-width, initial-scale=1"/>
                <AutoReload options=options.clone() />
                <HydrationScripts options/>
                <MetaTags/>
            </head>
            <body>
                <App/>
            </body>
        </html>
    }
}

#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();

    view! {
        <Stylesheet id="leptos" href="/pkg/screen_share.css"/>
        <Title text="Compartilhamento de tela"/>
        <Router>
            <main>
                <Routes fallback=|| view! { <p>"Página não encontrada."</p> }>
                    <Route path=path!("/") view=HomePlaceholder/>
                </Routes>
            </main>
        </Router>
    }
}

#[component]
fn HomePlaceholder() -> impl IntoView {
    view! { <h1>"Compartilhamento de tela"</h1> }
}
```

(Mantenha os imports/usos de `AutoReload` e `HydrationScripts` como já estiverem no `main.rs`/`lib.rs` gerados pelo template — só ajuste para chamar essa `shell` e esse `App`.)

- [ ] **Step 5: Verificar manualmente**

Run: `cargo leptos watch`, depois em outro terminal:
```bash
curl -s http://127.0.0.1:3000/ | grep -o 'Compartilhamento de tela'
```
Expected: a saída contém `Compartilhamento de tela`.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "chore: scaffold cargo-leptos project with placeholder home page"
```

---

## Task 2: Protocolo de sinalização (tipos de mensagem)

**Files:**
- Create: `src/signaling/mod.rs`
- Create: `src/signaling/protocol.rs`
- Test: incluído como `#[cfg(test)] mod tests` dentro de `src/signaling/protocol.rs`

**Interfaces:**
- Produces: `crate::signaling::protocol::{ClientMessage, ServerMessage}` — enums `Serialize`/`Deserialize` usados por todas as tasks seguintes (backend e frontend) para falar o mesmo protocolo JSON.

- [ ] **Step 1: Declarar o módulo**

`src/signaling/mod.rs`:
```rust
pub mod protocol;
```

Em `src/lib.rs`, adicione (junto aos demais `pub mod`):
```rust
pub mod signaling;
```

- [ ] **Step 2: Escrever o teste que falha**

`src/signaling/protocol.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn join_message_round_trips_through_json() {
        let msg = ClientMessage::Join { room: "ABCD1234".to_string() };
        let json = serde_json::to_string(&msg).unwrap();
        assert_eq!(json, r#"{"type":"join","room":"ABCD1234"}"#);

        let parsed: ClientMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, msg);
    }

    #[test]
    fn offer_server_message_round_trips_through_json() {
        let msg = ServerMessage::Offer { from: "peer-1".to_string(), sdp: "v=0...".to_string() };
        let json = serde_json::to_string(&msg).unwrap();
        assert_eq!(json, r#"{"type":"offer","from":"peer-1","sdp":"v=0..."}"#);

        let parsed: ServerMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, msg);
    }
}
```

- [ ] **Step 3: Rodar o teste e confirmar que falha (nem compila ainda)**

Run: `cargo test --lib signaling::protocol`
Expected: FAIL — `ClientMessage`/`ServerMessage` não existem.

- [ ] **Step 4: Implementar os tipos**

No topo de `src/signaling/protocol.rs`, antes do `#[cfg(test)]`:
```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    CreateRoom,
    Join { room: String },
    Offer { to: String, sdp: String },
    Answer { to: String, sdp: String },
    IceCandidate {
        to: String,
        candidate: String,
        sdp_mid: Option<String>,
        sdp_m_line_index: Option<u16>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    RoomCreated { room: String, peer_id: String },
    Joined { peer_id: String },
    RoomNotFound,
    PeerJoined { peer_id: String },
    PeerLeft { peer_id: String },
    RoomClosed,
    Offer { from: String, sdp: String },
    Answer { from: String, sdp: String },
    IceCandidate {
        from: String,
        candidate: String,
        sdp_mid: Option<String>,
        sdp_m_line_index: Option<u16>,
    },
}
```

- [ ] **Step 5: Rodar o teste e confirmar que passa**

Run: `cargo test --lib signaling::protocol`
Expected: PASS (2 testes).

- [ ] **Step 6: Commit**

```bash
git add src/signaling/mod.rs src/signaling/protocol.rs src/lib.rs
git commit -m "feat: define signaling protocol message types"
```

---

## Task 3: Registro de salas em memória (join/leave/relay)

**Files:**
- Create: `src/signaling/registry.rs`
- Modify: `src/signaling/mod.rs`
- Modify: `Cargo.toml` (adicionar dependência `uuid`)
- Test: incluído como `#[cfg(test)] mod tests` dentro de `src/signaling/registry.rs`

**Interfaces:**
- Consumes: `crate::signaling::protocol::ServerMessage` (Task 2).
- Produces: `crate::signaling::registry::Registry` com `new() -> Self`, `create_room(&self, sender: UnboundedSender<ServerMessage>) -> (String, String)` (retorna `(room_code, host_peer_id)`), `join_room(&self, room_code: &str, sender: UnboundedSender<ServerMessage>) -> Option<(String, String)>` (retorna `(peer_id, host_peer_id)` ou `None` se a sala não existir), `relay(&self, room_code: &str, to: &str, message: ServerMessage)`, `leave_room(&self, room_code: &str, peer_id: &str)`. `Registry` é `Clone` (compartilha estado via `Arc`).

- [ ] **Step 1: Adicionar a dependência `uuid`**

```bash
cargo add uuid --features v4
```

- [ ] **Step 2: Declarar o módulo (só compilado no lado servidor)**

`src/signaling/mod.rs`:
```rust
pub mod protocol;

#[cfg(feature = "ssr")]
pub mod registry;
```

- [ ] **Step 3: Escrever os testes que falham**

`src/signaling/registry.rs`:
```rust
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

        registry.relay(&room_code, &viewer_id, ServerMessage::Offer {
            from: host_id.clone(),
            sdp: "sdp-data".to_string(),
        });

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
        let (viewer_id, _) = registry.join_room(&room_code, viewer_tx).unwrap();
        viewer_rx.recv().await.unwrap(); // drena o Joined seria noutro canal; aqui só garantimos que a sala existe

        registry.leave_room(&room_code, &host_id);

        let notification = viewer_rx.recv().await.unwrap();
        assert_eq!(notification, ServerMessage::RoomClosed);

        let (_second_viewer_tx, _second_viewer_rx) = unbounded_channel();
        assert!(registry.join_room(&room_code, unbounded_channel().0).is_none());
        let _ = viewer_id;
    }
}
```

- [ ] **Step 4: Rodar os testes e confirmar que falham**

Run: `cargo test --lib --features ssr signaling::registry`
Expected: FAIL — `Registry` não existe.

- [ ] **Step 5: Implementar o registro**

No topo de `src/signaling/registry.rs`, antes do `#[cfg(test)]`:
```rust
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use rand::Rng;
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
    let mut rng = rand::thread_rng();
    (0..8)
        .map(|_| CHARS[rng.gen_range(0..CHARS.len())] as char)
        .collect()
}
```

Adicione `rand` como dependência se ainda não estiver presente: `cargo add rand`.

- [ ] **Step 6: Rodar os testes e confirmar que passam**

Run: `cargo test --lib --features ssr signaling::registry`
Expected: PASS (6 testes).

- [ ] **Step 7: Commit**

```bash
git add src/signaling/registry.rs src/signaling/mod.rs Cargo.toml Cargo.lock
git commit -m "feat: add in-memory room registry with join/leave/relay logic"
```

---

## Task 4: Endpoint WebSocket `/ws` (liga o registry ao Axum)

**Files:**
- Create: `src/signaling/ws.rs`
- Modify: `src/signaling/mod.rs`
- Modify: `src/main.rs` (registrar a rota `/ws` e o estado `Registry`)
- Modify: `Cargo.toml` (adicionar `tokio-tungstenite` e `futures-util` como dev-dependencies)
- Test: `tests/signaling_ws.rs`

**Interfaces:**
- Consumes: `crate::signaling::registry::Registry` (Task 3), `crate::signaling::protocol::{ClientMessage, ServerMessage}` (Task 2).
- Produces: `crate::signaling::ws::ws_handler` — handler Axum `async fn(State<Registry>, WebSocketUpgrade) -> impl IntoResponse`, montado em `/ws`.

- [ ] **Step 1: Adicionar dependências de teste**

```bash
cargo add tokio-tungstenite futures-util --dev
```

- [ ] **Step 2: Escrever o teste de integração que falha**

`tests/signaling_ws.rs`:
```rust
use futures_util::{SinkExt, StreamExt};
use screen_share::signaling::protocol::{ClientMessage, ServerMessage};
use tokio_tungstenite::tungstenite::Message;

async fn spawn_test_server() -> String {
    use axum::Router;
    use axum::routing::get;
    use screen_share::signaling::registry::Registry;
    use screen_share::signaling::ws::ws_handler;

    let registry = Registry::new();
    let app = Router::new()
        .route("/ws", get(ws_handler))
        .with_state(registry);

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
```

- [ ] **Step 3: Rodar o teste e confirmar que falha**

Run: `cargo test --features ssr --test signaling_ws`
Expected: FAIL — `screen_share::signaling::ws` não existe.

- [ ] **Step 4: Implementar o handler**

`src/signaling/ws.rs`:
```rust
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::IntoResponse;
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;

use super::protocol::{ClientMessage, ServerMessage};
use super::registry::Registry;

pub async fn ws_handler(State(registry): State<Registry>, ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, registry))
}

async fn handle_socket(socket: WebSocket, registry: Registry) {
    let (mut ws_sender, mut ws_receiver) = socket.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<ServerMessage>();

    let mut send_task = tokio::spawn(async move {
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
            ClientMessage::CreateRoom => {
                let (code, id) = registry.create_room(tx.clone());
                let _ = tx.send(ServerMessage::RoomCreated { room: code.clone(), peer_id: id.clone() });
                room_code = Some(code);
                peer_id = Some(id);
            }
            ClientMessage::Join { room } => match registry.join_room(&room, tx.clone()) {
                Some((id, _host)) => {
                    let _ = tx.send(ServerMessage::Joined { peer_id: id.clone() });
                    room_code = Some(room);
                    peer_id = Some(id);
                }
                None => {
                    let _ = tx.send(ServerMessage::RoomNotFound);
                }
            },
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
            ClientMessage::IceCandidate { to, candidate, sdp_mid, sdp_m_line_index } => {
                if let (Some(room), Some(from)) = (&room_code, &peer_id) {
                    registry.relay(
                        room,
                        &to,
                        ServerMessage::IceCandidate { from: from.clone(), candidate, sdp_mid, sdp_m_line_index },
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

`src/signaling/mod.rs`:
```rust
pub mod protocol;

#[cfg(feature = "ssr")]
pub mod registry;

#[cfg(feature = "ssr")]
pub mod ws;
```

Em `src/main.rs`, no bloco `#[cfg(feature = "ssr")]`, monte a rota antes das rotas do Leptos e injete o `Registry` como estado:
```rust
use screen_share::signaling::registry::Registry;
use screen_share::signaling::ws::ws_handler;

// ... dentro de main(), antes de construir o Router final:
let signaling_state = Registry::new();

let app = Router::new()
    .route("/ws", axum::routing::get(ws_handler))
    .with_state(signaling_state)
    // ... encadeie aqui o restante do setup de rotas do Leptos já gerado pelo template
    ;
```

(O template `start-axum` já monta as rotas do Leptos com seu próprio `Router`; combine os dois `Router`s com `.merge(...)` ou registre `/ws` num sub-`Router` próprio antes de mesclar, preservando o `LeptosOptions` como estado das rotas do Leptos — a rota `/ws` usa `Registry` como estado próprio, não `LeptosOptions`.)

- [ ] **Step 5: Rodar o teste e confirmar que passa**

Run: `cargo test --features ssr --test signaling_ws`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/signaling/ws.rs src/signaling/mod.rs src/main.rs tests/signaling_ws.rs Cargo.toml Cargo.lock
git commit -m "feat: wire signaling registry to /ws websocket endpoint"
```

---

## Task 5: Esqueleto de rotas do frontend + cliente WebSocket

**Files:**
- Modify: `src/app.rs` (rotas `/` e `/r/:code` apontando para componentes reais)
- Create: `src/pages/mod.rs`
- Create: `src/pages/home.rs` (stub)
- Create: `src/pages/room.rs` (stub)
- Create: `src/client/mod.rs`
- Create: `src/client/socket.rs`
- Modify: `Cargo.toml` (adicionar `web-sys`, `wasm-bindgen`, `wasm-bindgen-futures`, `js-sys`, `console_error_panic_hook` com as features necessárias, todos como dependências opcionais só do lado `hydrate`)

**Interfaces:**
- Consumes: `crate::signaling::protocol::{ClientMessage, ServerMessage}` (Task 2).
- Produces: `crate::client::socket::WsClient` com `connect(path: &str, on_message: impl Fn(ServerMessage) + 'static) -> Result<Self, wasm_bindgen::JsValue>`, `send(&self, msg: &ClientMessage)`, `on_open(&self, callback: impl FnOnce() + 'static)`. `crate::pages::home::HomePage`, `crate::pages::room::RoomPage` como componentes Leptos usados pelas rotas.

- [ ] **Step 1: Adicionar dependências do lado do navegador**

```bash
cargo add wasm-bindgen wasm-bindgen-futures js-sys
cargo add web-sys --features "WebSocket,MessageEvent,MediaDevices,MediaStream,MediaStreamConstraints,MediaStreamTrack,Navigator,Window,RtcPeerConnection,RtcConfiguration,RtcIceServer,RtcSdpType,RtcSessionDescriptionInit,RtcIceCandidateInit,RtcIceCandidate,RtcPeerConnectionIceEvent,RtcTrackEvent,RtcIceConnectionState,HtmlVideoElement,Location"
```

No `Cargo.toml`, mova essas quatro dependências para dentro do bloco condicional já existente na seção `[features]` do template: garanta que `hydrate = ["leptos/hydrate", "dep:web-sys", "dep:wasm-bindgen", "dep:wasm-bindgen-futures", "dep:js-sys"]` (ajuste os nomes exatos de feature conforme o `cargo add` os inseriu como `optional = true`).

- [ ] **Step 2: Criar os módulos de página (stubs)**

`src/pages/mod.rs`:
```rust
pub mod home;
pub mod room;
```

`src/pages/home.rs`:
```rust
use leptos::prelude::*;

#[component]
pub fn HomePage() -> impl IntoView {
    view! { <h1>"Compartilhar tela"</h1> }
}
```

`src/pages/room.rs`:
```rust
use leptos::prelude::*;
use leptos_router::hooks::use_params_map;

#[component]
pub fn RoomPage() -> impl IntoView {
    let params = use_params_map();
    let code = move || params.read().get("code").unwrap_or_default();

    view! { <h1>"Assistindo sala " {code}</h1> }
}
```

- [ ] **Step 3: Ligar as rotas reais em `src/app.rs`**

Troque a rota única e o `HomePlaceholder` do Task 1 por:
```rust
use crate::pages::home::HomePage;
use crate::pages::room::RoomPage;

// dentro de <Routes fallback=...>
<Route path=path!("/") view=HomePage/>
<Route path=path!("/r/:code") view=RoomPage/>
```

Remova o componente `HomePlaceholder` (não é mais usado).

Em `src/lib.rs`, adicione:
```rust
pub mod pages;

#[cfg(feature = "hydrate")]
pub mod client;
```

- [ ] **Step 4: Implementar o cliente WebSocket**

`src/client/mod.rs`:
```rust
pub mod socket;
```

`src/client/socket.rs`:
```rust
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{MessageEvent, WebSocket};

use crate::signaling::protocol::{ClientMessage, ServerMessage};

pub struct WsClient {
    socket: WebSocket,
    _on_message: Closure<dyn FnMut(MessageEvent)>,
}

impl WsClient {
    pub fn connect(path: &str, on_message: impl Fn(ServerMessage) + 'static) -> Result<Self, JsValue> {
        let location = web_sys::window().unwrap().location();
        let protocol = if location.protocol()? == "https:" { "wss" } else { "ws" };
        let host = location.host()?;
        let url = format!("{protocol}://{host}{path}");

        let socket = WebSocket::new(&url)?;

        let on_message_cb = Closure::<dyn FnMut(MessageEvent)>::new(move |event: MessageEvent| {
            if let Some(text) = event.data().as_string() {
                if let Ok(msg) = serde_json::from_str::<ServerMessage>(&text) {
                    on_message(msg);
                }
            }
        });
        socket.set_onmessage(Some(on_message_cb.as_ref().unchecked_ref()));

        Ok(Self { socket, _on_message: on_message_cb })
    }

    pub fn send(&self, msg: &ClientMessage) {
        if let Ok(json) = serde_json::to_string(msg) {
            let _ = self.socket.send_with_str(&json);
        }
    }

    pub fn on_open(&self, callback: impl FnOnce() + 'static) {
        let cb = Closure::once_into_js(callback);
        self.socket.set_onopen(Some(cb.as_ref().unchecked_ref()));
    }

    pub fn on_close(&self, callback: impl FnOnce() + 'static) {
        let cb = Closure::once_into_js(callback);
        self.socket.set_onclose(Some(cb.as_ref().unchecked_ref()));
    }
}
```

- [ ] **Step 5: Verificar manualmente no navegador**

Run: `cargo leptos watch`
Expected: abrir `http://127.0.0.1:3000/` mostra "Compartilhar tela"; abrir `http://127.0.0.1:3000/r/TESTE123` mostra "Assistindo sala TESTE123". Nenhum erro no console do navegador.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "feat: add routing skeleton and websocket client wrapper"
```

---

## Task 6: Fluxo de quem compartilha (captura de tela + criação de sala + oferta WebRTC)

**Files:**
- Create: `src/client/webrtc.rs`
- Modify: `src/client/mod.rs`
- Modify: `src/pages/home.rs`

**Interfaces:**
- Consumes: `crate::client::socket::WsClient` (Task 5), `crate::signaling::protocol::{ClientMessage, ServerMessage}` (Task 2).
- Produces: `crate::client::webrtc::{capture_display, new_peer_connection, create_offer, accept_answer, add_ice_candidate}` — usadas também pela Task 7.

- [ ] **Step 1: Implementar os helpers de WebRTC**

`src/client/webrtc.rs`:
```rust
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    MediaStream, MediaStreamConstraints, RtcConfiguration, RtcIceServer, RtcPeerConnection,
    RtcSdpType, RtcSessionDescriptionInit, RtcIceCandidateInit,
};

pub async fn capture_display() -> Result<MediaStream, JsValue> {
    let window = web_sys::window().ok_or_else(|| JsValue::from_str("sem window"))?;
    let media_devices = window.navigator().media_devices()?;

    let constraints = MediaStreamConstraints::new();
    constraints.set_video(&JsValue::TRUE);

    let promise = media_devices.get_display_media_with_constraints(&constraints)?;
    let stream = JsFuture::from(promise).await?;
    stream.dyn_into::<MediaStream>()
}

pub fn new_peer_connection() -> Result<RtcPeerConnection, JsValue> {
    let ice_server = RtcIceServer::new();
    let urls = js_sys::Array::new();
    urls.push(&JsValue::from_str("stun:stun.l.google.com:19302"));
    ice_server.set_urls(&JsValue::from(urls));

    let servers = js_sys::Array::new();
    servers.push(&ice_server);

    let config = RtcConfiguration::new();
    config.set_ice_servers(&servers);

    RtcPeerConnection::new_with_configuration(&config)
}

pub async fn create_offer(pc: &RtcPeerConnection) -> Result<String, JsValue> {
    let offer = JsFuture::from(pc.create_offer()).await?;
    let sdp = js_sys::Reflect::get(&offer, &JsValue::from_str("sdp"))?
        .as_string()
        .ok_or_else(|| JsValue::from_str("offer sem sdp"))?;

    let desc = RtcSessionDescriptionInit::new(RtcSdpType::Offer);
    desc.set_sdp(&sdp);
    JsFuture::from(pc.set_local_description(&desc)).await?;

    Ok(sdp)
}

pub async fn accept_answer(pc: &RtcPeerConnection, answer_sdp: &str) -> Result<(), JsValue> {
    let remote_desc = RtcSessionDescriptionInit::new(RtcSdpType::Answer);
    remote_desc.set_sdp(answer_sdp);
    JsFuture::from(pc.set_remote_description(&remote_desc)).await?;
    Ok(())
}

pub fn add_ice_candidate(
    pc: &RtcPeerConnection,
    candidate: &str,
    sdp_mid: Option<String>,
    sdp_m_line_index: Option<u16>,
) {
    let init = RtcIceCandidateInit::new(candidate);
    init.set_sdp_mid(sdp_mid.as_deref());
    init.set_sdp_m_line_index(sdp_m_line_index);
    let _ = pc.add_ice_candidate_with_opt_rtc_ice_candidate_init(Some(&init));
}
```

`src/client/mod.rs`:
```rust
pub mod socket;
pub mod webrtc;
```

- [ ] **Step 2: Implementar o fluxo de quem compartilha em `src/pages/home.rs`**

```rust
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use leptos::prelude::*;
use leptos::task::spawn_local;
use wasm_bindgen::JsCast;
use web_sys::{MediaStream, RtcPeerConnection, RtcPeerConnectionIceEvent};

use crate::client::socket::WsClient;
use crate::client::webrtc::{add_ice_candidate, capture_display, create_offer, new_peer_connection};
use crate::signaling::protocol::{ClientMessage, ServerMessage};

#[component]
pub fn HomePage() -> impl IntoView {
    let (status, set_status) = signal("Pronto para compartilhar.".to_string());
    let (room_link, set_room_link) = signal(None::<String>);

    let ws_slot: Rc<RefCell<Option<WsClient>>> = Rc::new(RefCell::new(None));
    let peers: Rc<RefCell<HashMap<String, RtcPeerConnection>>> = Rc::new(RefCell::new(HashMap::new()));
    let local_stream: Rc<RefCell<Option<MediaStream>>> = Rc::new(RefCell::new(None));
    let self_id: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));

    let start_sharing = move |_| {
        let ws_slot = ws_slot.clone();
        let peers = peers.clone();
        let local_stream = local_stream.clone();
        let self_id = self_id.clone();

        set_status.set("Selecione a tela para compartilhar...".to_string());

        spawn_local(async move {
            let stream = match capture_display().await {
                Ok(stream) => stream,
                Err(_) => {
                    set_status.set("Pronto para compartilhar.".to_string());
                    return;
                }
            };
            *local_stream.borrow_mut() = Some(stream);
            set_status.set("Conectando...".to_string());

            let ws_slot_for_messages = ws_slot.clone();
            let peers_for_messages = peers.clone();
            let local_stream_for_messages = local_stream.clone();
            let self_id_for_messages = self_id.clone();
            let set_status_for_messages = set_status;
            let set_room_link_for_messages = set_room_link;

            let on_message = move |msg: ServerMessage| {
                let ws_slot = ws_slot_for_messages.clone();
                let peers = peers_for_messages.clone();
                let local_stream = local_stream_for_messages.clone();
                let self_id = self_id_for_messages.clone();

                match msg {
                    ServerMessage::RoomCreated { room, peer_id } => {
                        *self_id.borrow_mut() = Some(peer_id);
                        let origin = web_sys::window().unwrap().location().origin().unwrap();
                        set_room_link_for_messages.set(Some(format!("{origin}/r/{room}")));
                        set_status_for_messages.set("Compartilhando! Envie o link para seus amigos.".to_string());
                    }
                    ServerMessage::PeerJoined { peer_id } => {
                        spawn_local(async move {
                            let Some(pc) = new_peer_connection().ok() else { return };

                            if let Some(stream) = local_stream.borrow().as_ref() {
                                for track in stream.get_tracks().iter() {
                                    let track: web_sys::MediaStreamTrack = track.dyn_into().unwrap();
                                    pc.add_track_0(&track, stream);
                                }
                            }

                            let target_id = peer_id.clone();
                            let ws_for_ice = ws_slot.clone();
                            let onicecandidate = wasm_bindgen::prelude::Closure::<dyn FnMut(RtcPeerConnectionIceEvent)>::new(
                                move |event: RtcPeerConnectionIceEvent| {
                                    if let Some(candidate) = event.candidate() {
                                        if let Some(ws) = ws_for_ice.borrow().as_ref() {
                                            ws.send(&ClientMessage::IceCandidate {
                                                to: target_id.clone(),
                                                candidate: candidate.candidate(),
                                                sdp_mid: candidate.sdp_mid(),
                                                sdp_m_line_index: candidate.sdp_m_line_index(),
                                            });
                                        }
                                    }
                                },
                            );
                            pc.set_onicecandidate(Some(onicecandidate.as_ref().unchecked_ref()));
                            onicecandidate.forget();

                            if let Ok(sdp) = create_offer(&pc).await {
                                if let Some(ws) = ws_slot.borrow().as_ref() {
                                    ws.send(&ClientMessage::Offer { to: peer_id.clone(), sdp });
                                }
                            }

                            peers.borrow_mut().insert(peer_id, pc);
                        });
                    }
                    ServerMessage::Answer { from, sdp } => {
                        if let Some(pc) = peers.borrow().get(&from).cloned() {
                            spawn_local(async move {
                                let _ = crate::client::webrtc::accept_answer(&pc, &sdp).await;
                            });
                        }
                    }
                    ServerMessage::IceCandidate { from, candidate, sdp_mid, sdp_m_line_index } => {
                        if let Some(pc) = peers.borrow().get(&from) {
                            add_ice_candidate(pc, &candidate, sdp_mid, sdp_m_line_index);
                        }
                    }
                    ServerMessage::PeerLeft { peer_id } => {
                        if let Some(pc) = peers.borrow_mut().remove(&peer_id) {
                            pc.close();
                        }
                    }
                    _ => {}
                }
            };

            match WsClient::connect("/ws", on_message) {
                Ok(ws) => {
                    ws.on_open({
                        let ws_slot = ws_slot.clone();
                        move || {
                            if let Some(ws) = ws_slot.borrow().as_ref() {
                                ws.send(&ClientMessage::CreateRoom);
                            }
                        }
                    });
                    *ws_slot.borrow_mut() = Some(ws);
                }
                Err(_) => set_status.set("Não foi possível conectar ao servidor.".to_string()),
            }
        });
    };

    view! {
        <div class="home">
            <h1>"Compartilhar tela"</h1>
            <button on:click=start_sharing>"Iniciar compartilhamento"</button>
            <p>{status}</p>
            <Show when=move || room_link.get().is_some()>
                <p>
                    "Link para convidar: "
                    <a href=move || room_link.get().unwrap_or_default()>
                        {move || room_link.get().unwrap_or_default()}
                    </a>
                </p>
            </Show>
        </div>
    }
}
```

Nota: `RtcPeerConnection::add_track_0` é o nome gerado pelo `web-sys` para a sobrecarga de `addTrack` com uma única `MediaStream` associada (a API JS aceita múltiplas streams via argumentos variádicos, e o `web-sys` numera as sobrecargas). Se o compilador indicar um nome diferente (ex.: `add_track`), use o nome que o `cargo doc -p web-sys --open` mostrar para a versão instalada — a lógica (chamar uma vez por track, passando a própria `stream`) não muda.

- [ ] **Step 3: Verificar manualmente no navegador**

Run: `cargo leptos watch`, abrir `http://127.0.0.1:3000/`.
Expected: clicar em "Iniciar compartilhamento" abre o seletor de tela do navegador; depois de escolher uma tela/janela, aparece "Compartilhando! Envie o link para seus amigos." com um link `/r/<código>`. Nenhum erro no console.

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "feat: implement sharer flow (display capture, room creation, per-viewer offer)"
```

---

## Task 7: Fluxo de quem assiste (entrar na sala + responder oferta + exibir vídeo)

**Files:**
- Modify: `src/pages/room.rs`

**Interfaces:**
- Consumes: `crate::client::socket::WsClient`, `crate::client::webrtc::{new_peer_connection, create_answer, add_ice_candidate}` (Tasks 5–6), `crate::signaling::protocol::{ClientMessage, ServerMessage}` (Task 2).
- Produces: `crate::client::webrtc::create_answer(pc: &RtcPeerConnection, offer_sdp: &str) -> Result<String, JsValue>` (nova função adicionada a `webrtc.rs` nesta task).

- [ ] **Step 1: Adicionar `create_answer` a `src/client/webrtc.rs`**

```rust
pub async fn create_answer(pc: &RtcPeerConnection, offer_sdp: &str) -> Result<String, JsValue> {
    let remote_desc = RtcSessionDescriptionInit::new(RtcSdpType::Offer);
    remote_desc.set_sdp(offer_sdp);
    JsFuture::from(pc.set_remote_description(&remote_desc)).await?;

    let answer = JsFuture::from(pc.create_answer()).await?;
    let sdp = js_sys::Reflect::get(&answer, &JsValue::from_str("sdp"))?
        .as_string()
        .ok_or_else(|| JsValue::from_str("answer sem sdp"))?;

    let local_desc = RtcSessionDescriptionInit::new(RtcSdpType::Answer);
    local_desc.set_sdp(&sdp);
    JsFuture::from(pc.set_local_description(&local_desc)).await?;

    Ok(sdp)
}
```

- [ ] **Step 2: Implementar o fluxo de espectador em `src/pages/room.rs`**

```rust
use std::cell::RefCell;
use std::rc::Rc;

use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::hooks::use_params_map;
use wasm_bindgen::JsCast;
use web_sys::{HtmlVideoElement, MediaStream, RtcPeerConnection, RtcPeerConnectionIceEvent, RtcTrackEvent};

use crate::client::socket::WsClient;
use crate::client::webrtc::{add_ice_candidate, create_answer, new_peer_connection};
use crate::signaling::protocol::{ClientMessage, ServerMessage};

#[component]
pub fn RoomPage() -> impl IntoView {
    let params = use_params_map();
    let room_code = move || params.read().get("code").unwrap_or_default();

    let (status, set_status) = signal("Conectando...".to_string());
    let video_ref = NodeRef::<leptos::html::Video>::new();

    let ws_slot: Rc<RefCell<Option<WsClient>>> = Rc::new(RefCell::new(None));
    let pc_slot: Rc<RefCell<Option<RtcPeerConnection>>> = Rc::new(RefCell::new(None));
    let host_id: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));

    Effect::new(move |_| {
        let room = room_code();
        let ws_slot = ws_slot.clone();
        let pc_slot = pc_slot.clone();
        let host_id = host_id.clone();

        let on_message = move |msg: ServerMessage| {
            let pc_slot = pc_slot.clone();
            let ws_slot = ws_slot.clone();
            let host_id = host_id.clone();

            match msg {
                ServerMessage::RoomNotFound => {
                    set_status.set("Sessão não encontrada ou já terminou.".to_string());
                }
                ServerMessage::Offer { from, sdp } => {
                    host_id.borrow_mut().replace(from.clone());
                    let video_ref = video_ref;

                    spawn_local(async move {
                        let Ok(pc) = new_peer_connection() else { return };

                        let ontrack = wasm_bindgen::prelude::Closure::<dyn FnMut(RtcTrackEvent)>::new(
                            move |event: RtcTrackEvent| {
                                if let Some(stream) = event.streams().get(0).dyn_into::<MediaStream>().ok() {
                                    if let Some(video) = video_ref.get() {
                                        let video: HtmlVideoElement = video.unchecked_into();
                                        video.set_src_object(Some(&stream));
                                    }
                                }
                            },
                        );
                        pc.set_ontrack(Some(ontrack.as_ref().unchecked_ref()));
                        ontrack.forget();

                        let ws_for_ice = ws_slot.clone();
                        let onicecandidate = wasm_bindgen::prelude::Closure::<dyn FnMut(RtcPeerConnectionIceEvent)>::new(
                            move |event: RtcPeerConnectionIceEvent| {
                                if let Some(candidate) = event.candidate() {
                                    if let Some(ws) = ws_for_ice.borrow().as_ref() {
                                        ws.send(&ClientMessage::IceCandidate {
                                            to: from.clone(),
                                            candidate: candidate.candidate(),
                                            sdp_mid: candidate.sdp_mid(),
                                            sdp_m_line_index: candidate.sdp_m_line_index(),
                                        });
                                    }
                                }
                            },
                        );
                        pc.set_onicecandidate(Some(onicecandidate.as_ref().unchecked_ref()));
                        onicecandidate.forget();

                        if let Ok(answer_sdp) = create_answer(&pc, &sdp).await {
                            if let Some(ws) = ws_slot.borrow().as_ref() {
                                ws.send(&ClientMessage::Answer { to: from.clone(), sdp: answer_sdp });
                            }
                        }

                        *pc_slot.borrow_mut() = Some(pc);
                        set_status.set("Conectado.".to_string());
                    });
                }
                ServerMessage::IceCandidate { candidate, sdp_mid, sdp_m_line_index, .. } => {
                    if let Some(pc) = pc_slot.borrow().as_ref() {
                        add_ice_candidate(pc, &candidate, sdp_mid, sdp_m_line_index);
                    }
                }
                ServerMessage::RoomClosed => {
                    set_status.set("O compartilhamento foi encerrado.".to_string());
                    if let Some(pc) = pc_slot.borrow_mut().take() {
                        pc.close();
                    }
                }
                _ => {}
            }
        };

        match WsClient::connect("/ws", on_message) {
            Ok(ws) => {
                ws.on_open({
                    let ws_slot = ws_slot.clone();
                    let room = room.clone();
                    move || {
                        if let Some(ws) = ws_slot.borrow().as_ref() {
                            ws.send(&ClientMessage::Join { room });
                        }
                    }
                });
                *ws_slot.borrow_mut() = Some(ws);
            }
            Err(_) => set_status.set("Não foi possível conectar ao servidor.".to_string()),
        }
    });

    view! {
        <div class="room">
            <h1>"Assistindo sala " {room_code}</h1>
            <p>{status}</p>
            <video node_ref=video_ref autoplay=true playsinline=true style="max-width: 100%;"></video>
        </div>
    }
}
```

- [ ] **Step 3: Verificar manualmente no navegador (ponta a ponta)**

Run: `cargo leptos watch`
1. Abra `http://127.0.0.1:3000/` numa aba, clique "Iniciar compartilhamento", escolha uma janela/tela.
2. Copie o link mostrado e abra em outra aba (ou outro navegador/máquina).
Expected: a segunda aba mostra "Conectado." e o `<video>` exibe a tela compartilhada da primeira aba, com poucos segundos de latência.

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "feat: implement viewer flow (join room, answer offer, render remote video)"
```

---

## Task 8: Tratamento de erros e reconexão

**Files:**
- Modify: `src/client/webrtc.rs` (adicionar `is_display_media_supported`)
- Modify: `src/pages/home.rs` (banner de navegador sem suporte, estado de falha de ICE por espectador)
- Modify: `src/pages/room.rs` (retry de WebSocket, estado de falha de ICE)

**Interfaces:**
- Produces: `crate::client::webrtc::is_display_media_supported() -> bool`.

- [ ] **Step 1: Detecção de suporte do navegador**

Adicione a `src/client/webrtc.rs`:
```rust
pub fn is_display_media_supported() -> bool {
    let Some(window) = web_sys::window() else { return false };
    let Ok(media_devices) = window.navigator().media_devices() else { return false };
    js_sys::Reflect::has(&media_devices, &JsValue::from_str("getDisplayMedia")).unwrap_or(false)
}
```

- [ ] **Step 2: Banner de "sem suporte" em `src/pages/home.rs`**

No topo do componente `HomePage`, antes de definir `start_sharing`:
```rust
let supported = crate::client::webrtc::is_display_media_supported();
```

No `view!`, envolva o botão:
```rust
<Show
    when=move || supported
    fallback=|| view! { <p>"Seu navegador não suporta compartilhamento de tela. Tente um navegador atualizado (Chrome, Edge, Firefox)."</p> }
>
    <button on:click=start_sharing>"Iniciar compartilhamento"</button>
</Show>
```

- [ ] **Step 3: Detectar falha de ICE por espectador em `src/pages/home.rs`**

No trecho de `ServerMessage::PeerJoined` (dentro do `spawn_local`, logo depois de criar `pc`), adicione:
```rust
let failed_peer_id = peer_id.clone();
let set_status_on_fail = set_status_for_messages;
let oniceconnectionstatechange = wasm_bindgen::prelude::Closure::<dyn FnMut()>::new({
    let pc_for_state = pc.clone();
    move || {
        if pc_for_state.ice_connection_state() == web_sys::RtcIceConnectionState::Failed {
            set_status_on_fail.set(format!("Não foi possível conectar com um espectador ({failed_peer_id})."));
        }
    }
});
pc.set_oniceconnectionstatechange(Some(oniceconnectionstatechange.as_ref().unchecked_ref()));
oniceconnectionstatechange.forget();
```

- [ ] **Step 4: Detectar falha de ICE em `src/pages/room.rs`**

No trecho de `ServerMessage::Offer` (dentro do `spawn_local`, logo depois de criar `pc`), adicione:
```rust
let oniceconnectionstatechange = wasm_bindgen::prelude::Closure::<dyn FnMut()>::new({
    let pc_for_state = pc.clone();
    move || {
        if pc_for_state.ice_connection_state() == web_sys::RtcIceConnectionState::Failed {
            set_status.set("Não foi possível conectar. Tente recarregar a página.".to_string());
        }
    }
});
pc.set_oniceconnectionstatechange(Some(oniceconnectionstatechange.as_ref().unchecked_ref()));
oniceconnectionstatechange.forget();
```

- [ ] **Step 5: Aviso de conexão perdida em `src/pages/room.rs`**

A spec original previa "tentar reconectar uma vez antes de avisar". Simplificamos para avisar direto, sem retry automático: como o `peer_id` é atribuído pelo servidor a cada conexão nova, uma reconexão silenciosa criaria um `peer_id` diferente e não teria como retomar a `RtcPeerConnection` já negociada — o resultado prático de um retry automático seria o mesmo que pedir para recarregar a página, só que mais lento. Recarregar a página já refaz o fluxo do zero corretamente.

Depois de `*ws_slot.borrow_mut() = Some(ws);` no bloco `Ok(ws) => { ... }` original, registre também `on_close` (na mesma variável `ws` antes de mover para o slot — reordene para chamar `ws.on_close(...)` e `ws.on_open(...)` antes de `*ws_slot.borrow_mut() = Some(ws)`):
```rust
ws.on_close(move || {
    set_status.set("Conexão perdida. Recarregue a página para tentar de novo.".to_string());
});
```

- [ ] **Step 6: Verificar manualmente**

1. Com o servidor parado, abra `/r/QUALQUER1` — Expected: fica em "Conectando..." e depois "Conexão perdida..." (o `onclose`/erro do WebSocket dispara).
2. Com o servidor rodando, abra `/r/CODIGOQUENAOEXISTE` depois de já ter uma sala ativa com outro código — Expected: "Sessão não encontrada ou já terminou."
3. No fluxo normal de compartilhamento, clique "Iniciar compartilhamento" e cancele o seletor de tela do navegador — Expected: volta para "Pronto para compartilhar." sem mensagem de erro assustadora.

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "feat: add error handling for unsupported browsers, ICE failures, and lost connections"
```

---

## Task 9: README com instruções de execução, checklist de teste manual e notas de deploy

**Files:**
- Create: `README.md`

**Interfaces:** nenhuma (documentação).

- [ ] **Step 1: Escrever o README**

`README.md`:
```markdown
# Compartilhamento de tela P2P

Site para compartilhar a tela com até 5 amigos ao mesmo tempo, direto do navegador
(Windows e Linux) — sem instalar nada, sem contas, sem áudio/chat. O vídeo trafega
P2P via WebRTC; o servidor só faz a sinalização inicial (troca de offer/answer/ICE).

## Rodando localmente

Pré-requisitos:
- Rust + `rustup target add wasm32-unknown-unknown`
- `cargo install cargo-leptos`

```bash
cargo leptos watch
```

Abra `http://127.0.0.1:3000/`.

## Testes automatizados

```bash
cargo test --features ssr
```

Cobre a lógica de sinalização (protocolo, registro de salas, endpoint WebSocket).
A captura de tela e o handshake WebRTC só existem dentro de um navegador real —
são validados manualmente (checklist abaixo).

## Checklist de teste manual (fluxo completo)

1. Abra `/` numa aba, clique "Iniciar compartilhamento", escolha uma janela/tela.
2. Confirme que aparece um link `/r/<código>`.
3. Abra esse link em outra aba (ou outra máquina) — confirme que o vídeo aparece
   em poucos segundos.
4. Abra o mesmo link numa terceira aba — confirme que ambos os espectadores
   recebem o vídeo simultaneamente.
5. Feche a aba de um espectador — confirme que os demais continuam recebendo
   vídeo normalmente.
6. Pare o compartilhamento (feche a aba de quem compartilha) — confirme que os
   espectadores restantes veem "O compartilhamento foi encerrado."
7. Abra um link com um código inexistente — confirme "Sessão não encontrada ou
   já terminou."

## Deploy

Este projeto compila para um único binário Rust. Em produção:

- Sirva atrás de HTTPS (obrigatório para `getDisplayMedia` e WebSocket seguro
  fora de `localhost`) — por exemplo, um reverse proxy como Caddy com TLS
  automático, ou uma plataforma que já termina TLS (Fly.io, Render).
- Não é necessário banco de dados nem armazenamento persistente — todo o
  estado de salas vive em memória e é descartado quando o processo reinicia.
- Sem TURN configurado (só STUN público). Se algum amigo estiver numa rede
  muito restritiva (CGNAT, firewall corporativo) e não conseguir conectar,
  isso é uma limitação conhecida da v1 — um servidor TURN (`coturn`) resolveria,
  mas fica fora de escopo por agora.
```

- [ ] **Step 2: Commit**

```bash
git add README.md
git commit -m "docs: add setup, testing checklist, and deployment notes"
```
