use leptos::prelude::*;
use leptos_router::hooks::use_params_map;

use crate::pages::status::status_meta;

#[derive(Clone, PartialEq)]
pub struct RoomMember {
    pub peer_id: String,
    pub nick: String,
    pub sharing: bool,
}

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
