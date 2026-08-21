use leptos::prelude::*;
use leptos_router::hooks::use_params_map;

use crate::pages::icons::{icon_eye, icon_log_out, icon_maximize, icon_screen_off};
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

    // Ver o comentário equivalente em `home.rs`: nick/cor sempre começam no
    // valor que o SSR também usaria, e o valor real do localStorage é
    // aplicado depois do mount — ler o perfil real já na criação do sinal
    // faz o binding `class:color-swatch--selected` hidratar errado e parar
    // de reagir a cliques nesse swatch específico.
    let (nick, set_nick) = signal(String::new());
    let (color, set_color) = signal(crate::pages::palette::DEFAULT_COLOR.to_string());
    load_profile_after_mount(set_nick, set_color);
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
    let watching = RwSignal::new(std::collections::HashSet::<String>::new());
    let expanded = RwSignal::new(None::<String>);
    let watchers_by_sharer = RwSignal::new(std::collections::HashMap::<String, Vec<String>>::new());
    let own_preview_hidden = RwSignal::new(false);
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
        watching,
        expanded,
        watchers_by_sharer,
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
        expanded,
        watchers_by_sharer,
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

    let toggle_share = share_toggle_handler(conn.clone(), is_sharing, set_is_sharing, set_status, local_video_ref);
    let leave_room = leave_room_handler(conn.clone());

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
                <button class="icon-btn icon-btn--danger" title="Sair da sala" aria-label="Sair da sala" on:click=leave_room>
                    {icon_log_out}
                </button>
            </div>
            <div class="grid" class:grid--focused=move || expanded.get().is_some()>
                {member_cards(conn, members, my_peer_id, is_sharing, watching, expanded, watchers_by_sharer, own_preview_hidden, local_video_ref, connection_errors)}
            </div>
        </div>
    }
}

#[cfg(not(feature = "hydrate"))]
fn load_profile_after_mount(_set_nick: WriteSignal<String>, _set_color: WriteSignal<String>) {}

#[cfg(feature = "hydrate")]
fn load_profile_after_mount(set_nick: WriteSignal<String>, set_color: WriteSignal<String>) {
    use leptos::task::spawn_local;

    spawn_local(async move {
        let profile = crate::client::storage::load_profile();
        set_nick.set(profile.nick);
        set_color.set(profile.color);
    });
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
    conn: RoomConnection,
    members: ReadSignal<Vec<RoomMember>>,
    my_peer_id: ReadSignal<Option<String>>,
    is_sharing: ReadSignal<bool>,
    watching: RwSignal<std::collections::HashSet<String>>,
    expanded: RwSignal<Option<String>>,
    watchers_by_sharer: RwSignal<std::collections::HashMap<String, Vec<String>>>,
    own_preview_hidden: RwSignal<bool>,
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
            let can_watch = move || {
                member_at().map(|m| m.sharing).unwrap_or(false) && !is_self() && !is_watching_this()
            };
            // `RoomMember.sharing` nunca fica `true` no PRÓPRIO card: o
            // servidor manda `PeerStartedSharing` só pros outros membros
            // (a pessoa já sabe localmente que está compartilhando, via
            // `is_sharing`). Pra decidir se mostra o selo de espectadores
            // no card de alguém, essa checagem tem que valer tanto pra
            // "outro membro cujo `sharing` é true" quanto pra "eu mesmo,
            // enquanto `is_sharing` estiver ligado".
            let member_is_sharing = move || {
                member_at().map(|m| m.sharing).unwrap_or(false) || (is_self() && is_sharing.get())
            };
            let is_expanded = move || {
                member_at().map(|m| expanded.get().as_deref() == Some(m.peer_id.as_str())).unwrap_or(false)
            };
            let own_preview_visible = move || is_self() && is_sharing.get() && !own_preview_hidden.get();
            // Com a Task 10, o vídeo do próprio card só aparece se não estiver
            // escondido — troca a definição de `showing_video` da Task 9.
            let showing_video = move || own_preview_visible() || (!is_self() && is_watching_this());
            let watcher_ids = move || {
                member_at().and_then(|m| watchers_by_sharer.get().get(&m.peer_id).cloned()).unwrap_or_default()
            };
            let watcher_names = move || {
                let ids = watcher_ids();
                let all_members = members.get();
                ids.iter()
                    .map(|id| {
                        all_members
                            .iter()
                            .find(|m| &m.peer_id == id)
                            .map(|m| m.nick.clone())
                            .unwrap_or_else(|| "alguém".to_string())
                    })
                    .collect::<Vec<_>>()
            };

            let watch = watch_click_handler(conn.clone(), members, watching, i);
            let stop_watch = stop_watching_click_handler(conn.clone(), members, watching, expanded, i);
            let toggle_preview_click = move |ev: leptos::ev::MouseEvent| {
                ev.stop_propagation();
                own_preview_hidden.update(|hidden| *hidden = !*hidden);
            };
            let fullscreen_click = move |ev: leptos::ev::MouseEvent| {
                ev.stop_propagation();
                let peer_id = member_at().map(|m| m.peer_id).unwrap_or_default();
                request_fullscreen(is_self(), &peer_id, local_video_ref);
            };
            // Clicar em qualquer lugar do banner (fora dos botões, que
            // interrompem a propagação) alterna foco: se já tem vídeo
            // visível — o próprio preview ou a transmissão de alguém que já
            // estamos assistindo —, expande; clicar de novo no card em foco
            // encolhe; clicar num card pequeno da tirinha que já mostra
            // vídeo troca o foco pra ele direto, sem precisar encolher antes.
            let toggle_focus_click = move |_: leptos::ev::MouseEvent| {
                if !showing_video() {
                    return;
                }
                let Some(member) = member_at() else { return };
                if is_expanded() {
                    expanded.set(None);
                } else {
                    expanded.set(Some(member.peer_id));
                }
            };

            view! {
                <div
                    class="card"
                    class:hidden=move || member_at().is_none()
                    class:card--focus=is_expanded
                    class:card--clickable=showing_video
                    style=move || {
                        let (border, bg) = member_at().map(|m| color_hex(&m.color)).unwrap_or(("#b0b8c1", "#2a2d31"));
                        format!("border-color: {border}; background-color: {bg};")
                    }
                    on:click=toggle_focus_click
                >
                    <div
                        class="watcher-badge"
                        class:hidden=move || !member_is_sharing()
                    >
                        {icon_eye}
                        <span>{move || watcher_ids().len()}</span>
                        <div class="watcher-badge__tooltip" class:hidden=move || watcher_names().is_empty()>
                            {move || watcher_names().join(", ")}
                        </div>
                    </div>
                    <div
                        class="card__avatar"
                        class:hidden=showing_video
                        style=move || {
                            let (border, bg) = member_at().map(|m| color_hex(&m.color)).unwrap_or(("#b0b8c1", "#2a2d31"));
                            format!("background-color: {bg}; border-color: {border};")
                        }
                    >
                        <span class="card__avatar-letter">
                            {move || member_at().map(|m| crate::pages::palette::avatar_letter(&m.nick)).unwrap_or_default()}
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
                    <button
                        class="card__watch-overlay"
                        class:hidden=move || !can_watch()
                        on:click=move |ev: leptos::ev::MouseEvent| {
                            ev.stop_propagation();
                            watch(ev);
                        }
                    >
                        <span class="card__watch-overlay-icon">"▶"</span>
                        <span>"Assistir compartilhamento"</span>
                    </button>
                    <div class="card__footer">
                        <span class="card__nick">{move || member_at().map(|m| m.nick).unwrap_or_default()}</span>
                        <div class="card__actions">
                            <button
                                class="icon-btn icon-btn--neutral"
                                class:hidden=move || !showing_video()
                                title="Tela cheia"
                                aria-label="Tela cheia"
                                on:click=fullscreen_click
                            >
                                {icon_maximize}
                            </button>
                            <button class="btn--ghost" class:hidden=move || !(is_self() && is_sharing.get()) on:click=toggle_preview_click>
                                {move || if own_preview_hidden.get() { "Mostrar preview" } else { "Esconder preview" }}
                            </button>
                            <button
                                class="icon-btn icon-btn--danger"
                                class:hidden=move || !is_watching_this()
                                title="Parar de assistir"
                                aria-label="Parar de assistir"
                                on:click=move |ev: leptos::ev::MouseEvent| {
                                    ev.stop_propagation();
                                    stop_watch(ev);
                                }
                            >
                                {icon_screen_off}
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
    _expanded: RwSignal<Option<String>>,
    _slot: usize,
) -> impl Fn(leptos::ev::MouseEvent) + Clone + 'static {
    move |_| {}
}

#[cfg(feature = "hydrate")]
fn stop_watching_click_handler(
    conn: RoomConnection,
    members: ReadSignal<Vec<RoomMember>>,
    watching: RwSignal<std::collections::HashSet<String>>,
    expanded: RwSignal<Option<String>>,
    slot: usize,
) -> impl Fn(leptos::ev::MouseEvent) + Clone + 'static {
    use crate::signaling::protocol::ClientMessage;

    move |_| {
        let Some(member) = members.get_untracked().get(slot).cloned() else { return };
        watching.update(|w| { w.remove(&member.peer_id); });
        // Se essa transmissão estava expandida, volta pra grade normal —
        // sem isso, o card em foco continuava vazio (sem vídeo) até a
        // pessoa clicar nele de novo pra encolher manualmente.
        expanded.update(|current| {
            if current.as_deref() == Some(member.peer_id.as_str()) {
                *current = None;
            }
        });
        if let Some(pc) = conn.incoming.borrow_mut().remove(&member.peer_id) {
            pc.close();
        }
        if let Some(ws) = conn.ws.borrow().as_ref() {
            ws.send(&ClientMessage::StopWatching { sharer_id: member.peer_id });
        }
    }
}

#[cfg(not(feature = "hydrate"))]
fn request_fullscreen(_is_self: bool, _peer_id: &str, _local_video_ref: NodeRef<leptos::html::Video>) {}

/// Coloca o `.card` inteiro em tela cheia — não só o `<video>`. Quando o
/// próprio `<video>` é o elemento em fullscreen, o Chrome injeta os
/// controles nativos de mídia por cima dele (barra de play/pause, seek),
/// como se fosse um player comum — errado aqui, já que uma transmissão de
/// tela ao vivo não é algo que faça sentido "pausar". Colocando o `.card`
/// (uma div comum) em fullscreen em vez do vídeo, esses controles nunca
/// aparecem, e como bônus o selo de espectadores e o rodapé com o nick —
/// que são irmãos do `<video>` dentro do card — continuam visíveis.
#[cfg(feature = "hydrate")]
fn request_fullscreen(is_self: bool, peer_id: &str, local_video_ref: NodeRef<leptos::html::Video>) {
    use wasm_bindgen::JsCast;

    let video: Option<web_sys::Element> = if is_self {
        local_video_ref.get_untracked().map(|v| {
            let video: web_sys::HtmlVideoElement = v.into();
            video.unchecked_into()
        })
    } else {
        web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.get_element_by_id(&format!("video-{peer_id}")))
    };
    let card = video.and_then(|v| v.closest(".card").ok().flatten());
    if let Some(card) = card {
        let _ = card.request_fullscreen();
    }
}

#[cfg(feature = "hydrate")]
fn apply_joined_snapshot(
    room_code: String,
    room_name: String,
    peer_id: String,
    joined_members: Vec<crate::signaling::protocol::MemberInfo>,
    active_sharers: Vec<String>,
    watcher_info: Vec<crate::signaling::protocol::WatcherInfo>,
    set_my_peer_id: WriteSignal<Option<String>>,
    set_members: WriteSignal<Vec<RoomMember>>,
    set_room_name: WriteSignal<Option<String>>,
    set_authenticated: WriteSignal<bool>,
    set_status: WriteSignal<String>,
    watchers_by_sharer: RwSignal<std::collections::HashMap<String, Vec<String>>>,
) {
    use std::collections::HashSet;

    use crate::client::storage::save_recent_room;
    use crate::profile::RecentRoom;

    let sharer_set: HashSet<String> = active_sharers.into_iter().collect();
    let members: Vec<RoomMember> = joined_members
        .into_iter()
        .map(|m| RoomMember { sharing: sharer_set.contains(&m.peer_id), peer_id: m.peer_id, nick: m.nick, color: m.color })
        .collect();
    watchers_by_sharer.set(watcher_info.into_iter().map(|w| (w.sharer_id, w.watchers)).collect());
    save_recent_room(RecentRoom { code: room_code, name: room_name.clone() });
    set_my_peer_id.set(Some(peer_id));
    set_members.set(members);
    set_room_name.set(Some(room_name));
    set_authenticated.set(true);
    set_status.set("Conectado.".to_string());
}

#[cfg(feature = "hydrate")]
fn build_message_handler(
    set_status: WriteSignal<String>,
    set_authenticated: WriteSignal<bool>,
    set_room_name: WriteSignal<Option<String>>,
    set_members: WriteSignal<Vec<RoomMember>>,
    set_my_peer_id: WriteSignal<Option<String>>,
    conn: RoomConnection,
    watching: RwSignal<std::collections::HashSet<String>>,
    expanded: RwSignal<Option<String>>,
    watchers_by_sharer: RwSignal<std::collections::HashMap<String, Vec<String>>>,
    connection_errors: RwSignal<std::collections::HashSet<String>>,
) -> impl Fn(crate::signaling::protocol::ServerMessage) + 'static {
    use leptos::task::spawn_local;
    use wasm_bindgen::JsCast;
    use web_sys::{MediaStream, RtcPeerConnectionIceEvent, RtcTrackEvent};

    use crate::client::webrtc::{accept_answer, add_ice_candidate, create_answer, create_offer, new_peer_connection};
    use crate::signaling::protocol::{ClientMessage, ServerMessage};

    move |msg: ServerMessage| match msg {
        ServerMessage::Joined { peer_id, room, room_name, members: joined_members, active_sharers, watcher_info } => {
            apply_joined_snapshot(
                room,
                room_name,
                peer_id,
                joined_members,
                active_sharers,
                watcher_info,
                set_my_peer_id,
                set_members,
                set_room_name,
                set_authenticated,
                set_status,
                watchers_by_sharer,
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
            expanded.update(|current| {
                if current.as_deref() == Some(peer_id.as_str()) {
                    *current = None;
                }
            });
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
            watching.update(|w| { w.remove(&peer_id); });
            expanded.update(|current| {
                if current.as_deref() == Some(peer_id.as_str()) {
                    *current = None;
                }
            });
            // Ninguém assiste quem parou de compartilhar — evita mostrar
            // uma contagem velha se essa pessoa voltar a compartilhar
            // antes do primeiro `WatchersChanged` novo chegar.
            watchers_by_sharer.update(|w| { w.remove(&peer_id); });
            if let Some(pc) = conn.incoming.borrow_mut().remove(&peer_id) {
                pc.close();
            }
        }
        ServerMessage::WatchersChanged { sharer_id, watchers } => {
            watchers_by_sharer.update(|w| { w.insert(sharer_id, watchers); });
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
        ServerMessage::WatchRequested { from } => {
            let conn = conn.clone();
            spawn_local(async move {
                let Ok(pc) = new_peer_connection() else { return };
                conn.outgoing.borrow_mut().insert(from.clone(), pc.clone());
                connection_errors.update(|errors| { errors.remove(&from); });

                if let Some(stream) = conn.local_stream.borrow().as_ref() {
                    for track in stream.get_tracks().iter() {
                        let track: web_sys::MediaStreamTrack = track.unchecked_into();
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
    }
}

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
    _expanded: RwSignal<Option<String>>,
    _watchers_by_sharer: RwSignal<std::collections::HashMap<String, Vec<String>>>,
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
    expanded: RwSignal<Option<String>>,
    watchers_by_sharer: RwSignal<std::collections::HashMap<String, Vec<String>>>,
    connection_errors: RwSignal<std::collections::HashSet<String>>,
) {
    use crate::client::session;

    let Some(mut session) = session::take(&room_code) else { return };

    let on_message = build_message_handler(set_status, set_authenticated, set_room_name, set_members, set_my_peer_id, conn.clone(), watching, expanded, watchers_by_sharer, connection_errors);
    session.ws.set_on_message(on_message);
    session.ws.on_close(move || {
        set_status.set("Conexão perdida. Recarregue a página para tentar de novo.".to_string());
    });

    // Uma sessão pendente vem sempre da home criando uma sala do zero —
    // nunca tem espectador nenhum ainda, então não precisa de
    // `watcher_info` vindo de lugar nenhum.
    apply_joined_snapshot(
        session.room,
        session.room_name,
        session.peer_id,
        session.members,
        session.active_sharers,
        Vec::new(),
        set_my_peer_id,
        set_members,
        set_room_name,
        set_authenticated,
        set_status,
        watchers_by_sharer,
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
    _expanded: RwSignal<Option<String>>,
    _watchers_by_sharer: RwSignal<std::collections::HashMap<String, Vec<String>>>,
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
    expanded: RwSignal<Option<String>>,
    watchers_by_sharer: RwSignal<std::collections::HashMap<String, Vec<String>>>,
    connection_errors: RwSignal<std::collections::HashSet<String>>,
) -> impl Fn(String, String, String) + Clone + 'static {
    use crate::client::socket::WsClient;
    use crate::client::storage::save_profile;
    use crate::profile::Profile;
    use crate::signaling::protocol::ClientMessage;

    move |nick: String, color: String, password: String| {
        let conn = conn.clone();
        let room_code = room_code.clone();
        set_status.set("Conectando...".to_string());

        let on_message = build_message_handler(set_status, set_authenticated, set_room_name, set_members, set_my_peer_id, conn.clone(), watching, expanded, watchers_by_sharer, connection_errors);

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
    _is_sharing: ReadSignal<bool>,
    _set_is_sharing: WriteSignal<bool>,
    _set_status: WriteSignal<String>,
    _local_video_ref: NodeRef<leptos::html::Video>,
) -> impl Fn(leptos::ev::MouseEvent) + Clone + 'static {
    move |_| {}
}

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
