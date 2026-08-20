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

    // Se viemos da criação da sala na home, a conexão já está autenticada
    // (ver `client::session`) — assume ela em vez de pedir nick/senha de
    // novo. Chamada direta (não via Effect): `initial_code` já está
    // disponível de forma síncrona na montagem do componente.
    adopt_pending_session(initial_code, conn.clone(), set_status, set_authenticated, set_members, set_my_peer_id, connection_errors);

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
                <span class="status-row__spacer"></span>
                // Assim como o portão de autenticação (topo do arquivo): o
                // botão fica sempre montado e alterna por `class:hidden`, não
                // por `<Show>` — `on:click` captura `toggle_share`, que
                // carrega um `RoomConnection` (`Rc<RefCell<...>>`, não
                // Send+Sync), e `<Show>` exige Send+Sync dos seus filhos.
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
) -> impl Fn(leptos::ev::MouseEvent) + Clone + 'static {
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
) -> impl Fn(leptos::ev::MouseEvent) + Clone + 'static {
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

            // A ordem importa: StartShare precisa sair antes das ofertas (abaixo)
            // para que o PeerStartedSharing chegue em cada espectador antes do
            // Offer correspondente — é o que garante que o tile <video> já
            // exista no DOM quando o ontrack tentar encontrá-lo.
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

                    // Mesmo princípio de isolamento do lado de quem assiste:
                    // se a conexão com ESSE espectador falhar, só o tile dele
                    // (do lado dele) fica com erro — não afeta os outros
                    // espectadores da minha transmissão.
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
