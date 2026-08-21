use leptos::prelude::*;
use leptos_router::hooks::use_params_map;

use crate::ui::components::color_picker::ColorPicker;
use crate::ui::components::icons::{
    icon_check, icon_eye, icon_eye_off, icon_link, icon_log_out, icon_maximize, icon_monitor, icon_pip, icon_screen_off, icon_video, icon_video_off,
};
use crate::ui::components::palette::color_hex;
use crate::ui::components::status::status_meta;
use crate::ui::components::status_message::StatusMessage;
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
    ws: std::rc::Rc<std::cell::RefCell<Option<crate::ui::client::socket::WsClient>>>,
    outgoing: std::rc::Rc<std::cell::RefCell<std::collections::HashMap<String, web_sys::RtcPeerConnection>>>,
    incoming: std::rc::Rc<std::cell::RefCell<std::collections::HashMap<String, web_sys::RtcPeerConnection>>>,
    local_stream: std::rc::Rc<std::cell::RefCell<Option<web_sys::MediaStream>>>,
    // Marcado antes de um close intencional; `on_close` (assíncrono, roda
    // depois) confere essa flag pra não sobrescrever a mensagem de status
    // já definida com o erro genérico de conexão perdida.
    expected_close: std::rc::Rc<std::cell::Cell<bool>>,
}

#[cfg(feature = "hydrate")]
impl RoomConnection {
    fn new() -> Self {
        Self {
            ws: Default::default(),
            outgoing: Default::default(),
            incoming: Default::default(),
            local_stream: Default::default(),
            expected_close: Default::default(),
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

    // Começa com o valor padrão do SSR; o real (localStorage) só chega
    // depois do mount, ou a hidratação do color-swatch selecionado quebra.
    let (nick, set_nick) = signal(String::new());
    let (color, set_color) = signal(crate::ui::components::palette::DEFAULT_COLOR.to_string());
    load_profile_after_mount(set_nick, set_color);
    let (password, set_password) = signal(String::new());
    let (status, set_status) = signal("Informe o nick e a senha da sala.".to_string());
    let (authenticated, set_authenticated) = signal(false);
    let (room_exists, set_room_exists) = signal(None::<bool>);
    let (room_name, set_room_name) = signal(None::<String>);
    let (members, set_members) = signal(Vec::<RoomMember>::new());
    let (my_peer_id, set_my_peer_id) = signal(None::<String>);
    let (is_sharing, set_is_sharing) = signal(false);
    let connection_errors = RwSignal::new(std::collections::HashSet::<String>::new());
    let watching = RwSignal::new(std::collections::HashSet::<String>::new());
    let expanded = RwSignal::new(None::<String>);
    let watchers_by_sharer = RwSignal::new(std::collections::HashMap::<String, Vec<String>>::new());
    let own_preview_hidden = RwSignal::new(false);
    let hide_idle = RwSignal::new(false);
    let controls_visible = RwSignal::new(true);
    let invite_copied = RwSignal::new(false);
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
        set_room_exists,
        my_peer_id,
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
        set_room_exists,
        my_peer_id,
    );

    start_room_check(initial_code.clone(), authenticated, set_room_exists, set_room_name);

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

    let toggle_share = share_toggle_handler(conn.clone(), is_sharing, set_is_sharing, own_preview_hidden, set_status, my_peer_id, expanded);
    let invite_click = invite_click_handler(initial_code.clone(), invite_copied);
    let leave_or_stop_watching = leave_or_stop_watching_handler(conn.clone(), watching, expanded, my_peer_id);
    let (pause_hide_controls, resume_hide_controls) = setup_auto_hide_controls(controls_visible);
    setup_adaptive_grid(members, hide_idle, own_preview_hidden, is_sharing, expanded);

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
            <a class="btn btn--ghost btn--block" href="/">"Voltar à página principal"</a>
        </div>
        // class:hidden em vez de `<Show>`: o Leptos 0.8 exige Send + Sync
        // nos filhos de `<Show>`, e o formulário captura um
        // `Rc<RefCell<WsClient>>`, que não é.
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
                <ColorPicker selected=color on_select=set_color/>
                <label class="field">
                    <span class="field__label">"Senha da sala"</span>
                    <input class="field__input" type="password" required prop:value=password
                        on:input:target=move |ev| set_password.set(ev.target().value())/>
                </label>
                <button class="btn btn--primary" type="submit">"Entrar"</button>
            </form>
            <StatusMessage status=status/>
        </div>
        <div class="room-page" class:hidden=move || !authenticated.get()>
            <div class="stage-header">
                <span class=lamp_class></span>
                <span class="status-row__meta">{move || room_name.get().unwrap_or_default()}</span>
                <span class="room-member-count">{move || format!("{}/{}", members.get().len(), MAX_MEMBERS)}</span>
                <span class="status-row__spacer"></span>
                <button
                    class="invite-btn"
                    class:invite-btn--copied=invite_copied
                    title="Copiar link de convite da sala"
                    aria-label="Copiar link de convite da sala"
                    on:click=invite_click
                >
                    {move || if invite_copied.get() { icon_check().into_any() } else { icon_link().into_any() }}
                    <span>{move || if invite_copied.get() { "Link copiado!" } else { "Convidar" }}</span>
                </button>
                <span class="status-text status-text--error" class:hidden=move || can_share>
                    "Seu navegador não suporta compartilhar tela — você ainda pode assistir."
                </span>
            </div>
            <div id="member-grid" class="grid" class:grid--focused=move || expanded.get().is_some()>
                {member_cards(conn, members, my_peer_id, is_sharing, watching, expanded, watchers_by_sharer, own_preview_hidden, hide_idle, connection_errors)}
            </div>
            <div
                class="room-controls"
                class:room-controls--hidden=move || !controls_visible.get()
                on:mouseenter=move |_| pause_hide_controls()
                on:mouseleave=move |_| resume_hide_controls()
            >
                <button
                    class="icon-btn"
                    class:icon-btn--danger=is_sharing
                    class:icon-btn--neutral=move || !is_sharing.get()
                    class:hidden=move || !can_share
                    title=move || if is_sharing.get() { "Parar de compartilhar" } else { "Compartilhar minha tela" }
                    aria-label="Compartilhar ou parar de compartilhar minha tela"
                    on:click=toggle_share.clone()
                >
                    {move || if is_sharing.get() { icon_screen_off().into_any() } else { icon_monitor().into_any() }}
                </button>
                <button
                    class="icon-btn icon-btn--neutral"
                    class:icon-btn--active=hide_idle
                    title=move || if hide_idle.get() { "Mostrar todo mundo" } else { "Ocultar quem não está transmitindo" }
                    aria-label="Ocultar quem não está transmitindo"
                    on:click=move |_| hide_idle.update(|v| *v = !*v)
                >
                    {icon_eye_off}
                </button>
                <button
                    class="icon-btn icon-btn--neutral"
                    class:icon-btn--active=own_preview_hidden
                    class:hidden=move || !is_sharing.get()
                    title=move || if own_preview_hidden.get() { "Mostrar meu preview" } else { "Esconder meu preview" }
                    aria-label="Esconder meu preview"
                    on:click=move |_| own_preview_hidden.update(|v| *v = !*v)
                >
                    {move || if own_preview_hidden.get() { icon_video().into_any() } else { icon_video_off().into_any() }}
                </button>
                <button
                    class="icon-btn icon-btn--danger"
                    title=move || if expanded.get().is_some() { "Parar de assistir" } else { "Sair da sala" }
                    aria-label="Sair da sala"
                    on:click=leave_or_stop_watching
                >
                    {icon_log_out}
                </button>
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
        let profile = crate::ui::client::storage::load_profile();
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

    use crate::ui::client::rooms_api::check_room;

    spawn_local(async move {
        let result = check_room(&room_code).await;
        // Sessão pendente já pode ter autenticado enquanto essa checagem
        // estava em voo — ignora nesse caso.
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

/// `MAX_MEMBERS` cards estáticos e fixos, não uma `<For>` reativa — os
/// botões capturam `RoomConnection` (`Rc<RefCell<...>>`, não Send + Sync,
/// exigido pelos filhos de `<For>` no Leptos 0.8). O slot `i` mostra quem
/// estiver na posição `i` de `members`, não um membro fixo.
fn member_cards(
    conn: RoomConnection,
    members: ReadSignal<Vec<RoomMember>>,
    my_peer_id: ReadSignal<Option<String>>,
    is_sharing: ReadSignal<bool>,
    watching: RwSignal<std::collections::HashSet<String>>,
    expanded: RwSignal<Option<String>>,
    watchers_by_sharer: RwSignal<std::collections::HashMap<String, Vec<String>>>,
    own_preview_hidden: RwSignal<bool>,
    hide_idle: RwSignal<bool>,
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
            // `RoomMember.sharing` nunca é `true` no próprio card — o
            // servidor só manda `PeerStartedSharing` pros outros.
            let member_is_sharing = move || {
                member_at().map(|m| m.sharing).unwrap_or(false) || (is_self() && is_sharing.get())
            };
            let is_expanded = move || {
                member_at().map(|m| expanded.get().as_deref() == Some(m.peer_id.as_str())).unwrap_or(false)
            };
            let own_preview_visible = move || is_self() && is_sharing.get() && !own_preview_hidden.get();
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
            let fullscreen_click = move |ev: leptos::ev::MouseEvent| {
                ev.stop_propagation();
                let peer_id = member_at().map(|m| m.peer_id).unwrap_or_default();
                toggle_fullscreen(is_self(), &peer_id);
            };
            let pip_click = move |ev: leptos::ev::MouseEvent| {
                ev.stop_propagation();
                let peer_id = member_at().map(|m| m.peer_id).unwrap_or_default();
                toggle_picture_in_picture(is_self(), &peer_id);
            };
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
                    class:hidden=move || {
                        let filtered_out_of_main_grid = expanded.get().is_none()
                            && ((hide_idle.get() && !member_is_sharing())
                                || (is_self() && own_preview_hidden.get()));
                        member_at().is_none() || filtered_out_of_main_grid
                    }
                    class:card--focus=is_expanded
                    class:card--clickable=showing_video
                    style=move || {
                        let (border, _bg) = member_at().map(|m| color_hex(&m.color)).unwrap_or(("#b0b8c1", "#2a2d31"));
                        format!("border-color: {border}; --member-accent: {border};")
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
                            let (border, _bg) = member_at().map(|m| color_hex(&m.color)).unwrap_or(("#b0b8c1", "#2a2d31"));
                            format!("background-color: color-mix(in srgb, {border} 22%, var(--surface-2)); border-color: {border};")
                        }
                    >
                        <span class="card__avatar-letter">
                            {move || member_at().map(|m| crate::ui::components::palette::avatar_letter(&m.nick)).unwrap_or_default()}
                        </span>
                    </div>
                    <video
                        id=move || member_at().map(|m| format!("video-self-{}", m.peer_id)).unwrap_or_default()
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
                            <button
                                class="icon-btn icon-btn--neutral"
                                class:hidden=move || !showing_video()
                                title="Picture-in-picture"
                                aria-label="Picture-in-picture"
                                on:click=pip_click
                            >
                                {icon_pip}
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
fn toggle_fullscreen(_is_self: bool, _peer_id: &str) {}

/// Coloca o `.card` (não o `<video>`) em fullscreen: se fosse o vídeo, o
/// Chrome injeta controles nativos de play/pause/seek por cima, sem sentido
/// numa transmissão ao vivo.
#[cfg(feature = "hydrate")]
fn toggle_fullscreen(is_self: bool, peer_id: &str) {
    let Some(document) = web_sys::window().and_then(|w| w.document()) else { return };

    if document.fullscreen_element().is_some() {
        let _ = document.exit_fullscreen();
        return;
    }

    let id = if is_self { format!("video-self-{peer_id}") } else { format!("video-{peer_id}") };
    let video = document.get_element_by_id(&id);
    let card = video.and_then(|v| v.closest(".card").ok().flatten());
    if let Some(card) = card {
        let _ = card.request_fullscreen();
    }
}

#[cfg(not(feature = "hydrate"))]
fn toggle_picture_in_picture(_is_self: bool, _peer_id: &str) {}

/// Mesmo esquema de `id` que `toggle_fullscreen` usa pra achar o vídeo certo
/// entre os slots fixos. Só um PiP por vez é limitação do navegador.
#[cfg(feature = "hydrate")]
fn toggle_picture_in_picture(is_self: bool, peer_id: &str) {
    use leptos::task::spawn_local;
    use wasm_bindgen::JsCast;
    use wasm_bindgen_futures::JsFuture;

    let Some(document) = web_sys::window().and_then(|w| w.document()) else { return };

    if document.picture_in_picture_element().is_some() {
        let promise = document.exit_picture_in_picture();
        spawn_local(async move {
            let _ = JsFuture::from(promise).await;
        });
        return;
    }

    let id = if is_self { format!("video-self-{peer_id}") } else { format!("video-{peer_id}") };
    let Some(video) = document.get_element_by_id(&id) else { return };
    let video: web_sys::HtmlVideoElement = video.unchecked_into();
    let promise = video.request_picture_in_picture();
    spawn_local(async move {
        let _ = JsFuture::from(promise).await;
    });
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

    use crate::ui::client::storage::save_recent_room;
    use crate::ui::profile::RecentRoom;

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
    set_room_exists: WriteSignal<Option<bool>>,
    my_peer_id: ReadSignal<Option<String>>,
) -> impl Fn(crate::signaling::protocol::ServerMessage) + 'static {
    use leptos::task::spawn_local;
    use wasm_bindgen::JsCast;
    use web_sys::{MediaStream, RtcPeerConnectionIceEvent, RtcTrackEvent};

    use crate::ui::client::webrtc::{accept_answer, add_ice_candidate, create_answer, create_offer, new_peer_connection};
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
        ServerMessage::Kicked => {
            // Mesmo dispositivo entrou nessa sala em outra aba — essa
            // conexão foi substituída. `room_exists` precisa ser forçado:
            // quem criou a sala nunca passou por `start_room_check`, então
            // esse sinal nunca foi preenchido e o portão ficaria preso em
            // "Verificando sala..." pra sempre.
            conn.expected_close.set(true);
            if let Some(ws) = conn.ws.borrow().as_ref() {
                ws.close();
            }
            set_authenticated.set(false);
            set_room_exists.set(Some(true));
            set_status.set("Você entrou nessa sala em outra aba ou janela — esta conexão foi encerrada.".to_string());
        }
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
                // Diferente do ramo `Offer`: aqui o par remoto é quem
                // assiste, não o dono da transmissão. `stream_owner` tem que
                // ser meu próprio peer_id, não `target_id`, ou o outro lado
                // guarda o candidato no mapa errado (`outgoing` em vez de
                // `incoming`) e a conexão nunca fecha o ICE.
                let stream_owner_id = my_peer_id.get_untracked().unwrap_or_else(|| target_id.clone());
                let conn_for_ice = conn.clone();
                let onicecandidate = wasm_bindgen::prelude::Closure::<dyn FnMut(RtcPeerConnectionIceEvent)>::new(move |event: RtcPeerConnectionIceEvent| {
                    if let Some(candidate) = event.candidate() {
                        if let Some(ws) = conn_for_ice.ws.borrow().as_ref() {
                            ws.send(&ClientMessage::IceCandidate {
                                to: target_id.clone(),
                                stream_owner: stream_owner_id.clone(),
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
    _set_room_exists: WriteSignal<Option<bool>>,
    _my_peer_id: ReadSignal<Option<String>>,
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
    set_room_exists: WriteSignal<Option<bool>>,
    my_peer_id: ReadSignal<Option<String>>,
) {
    use crate::ui::client::session;

    let Some(mut session) = session::take(&room_code) else { return };

    let on_message = build_message_handler(set_status, set_authenticated, set_room_name, set_members, set_my_peer_id, conn.clone(), watching, expanded, watchers_by_sharer, connection_errors, set_room_exists, my_peer_id);
    session.ws.set_on_message(on_message);
    session.ws.on_close({
        let conn = conn.clone();
        move || {
            if !conn.expected_close.get() {
                set_status.set("Conexão perdida. Recarregue a página para tentar de novo.".to_string());
            }
        }
    });

    // Sessão pendente vem sempre da home criando uma sala nova — nunca tem
    // espectador ainda.
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
    _set_room_exists: WriteSignal<Option<bool>>,
    _my_peer_id: ReadSignal<Option<String>>,
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
    set_room_exists: WriteSignal<Option<bool>>,
    my_peer_id: ReadSignal<Option<String>>,
) -> impl Fn(String, String, String) + Clone + 'static {
    use crate::ui::client::socket::WsClient;
    use crate::ui::client::storage::{ensure_device_id, save_profile};
    use crate::ui::profile::Profile;
    use crate::signaling::protocol::ClientMessage;

    move |nick: String, color: String, password: String| {
        let conn = conn.clone();
        conn.expected_close.set(false);
        let room_code = room_code.clone();
        set_status.set("Conectando...".to_string());

        let on_message = build_message_handler(set_status, set_authenticated, set_room_name, set_members, set_my_peer_id, conn.clone(), watching, expanded, watchers_by_sharer, connection_errors, set_room_exists, my_peer_id);

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
                            ws.send(&ClientMessage::JoinRoom {
                                room: room_code.clone(),
                                nick: nick.clone(),
                                password: password.clone(),
                                color: color.clone(),
                                device_id: ensure_device_id(),
                            });
                        }
                    }
                });
                ws.on_close({
                    let conn = conn.clone();
                    move || {
                        if !conn.expected_close.get() {
                            set_status.set("Conexão perdida. Recarregue a página para tentar de novo.".to_string());
                        }
                    }
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
    crate::ui::client::webrtc::is_display_media_supported()
}

#[cfg(not(feature = "hydrate"))]
fn share_toggle_handler(
    _conn: RoomConnection,
    _is_sharing: ReadSignal<bool>,
    _set_is_sharing: WriteSignal<bool>,
    _own_preview_hidden: RwSignal<bool>,
    _set_status: WriteSignal<String>,
    _my_peer_id: ReadSignal<Option<String>>,
    _expanded: RwSignal<Option<String>>,
) -> impl Fn(leptos::ev::MouseEvent) + Clone + 'static {
    move |_| {}
}

#[cfg(feature = "hydrate")]
fn share_toggle_handler(
    conn: RoomConnection,
    is_sharing: ReadSignal<bool>,
    set_is_sharing: WriteSignal<bool>,
    own_preview_hidden: RwSignal<bool>,
    set_status: WriteSignal<String>,
    my_peer_id: ReadSignal<Option<String>>,
    expanded: RwSignal<Option<String>>,
) -> impl Fn(leptos::ev::MouseEvent) + Clone + 'static {
    use leptos::task::spawn_local;
    use wasm_bindgen::JsCast;
    use web_sys::MediaStreamTrack;

    use crate::ui::client::webrtc::capture_display;
    use crate::signaling::protocol::ClientMessage;

    move |_| {
        if is_sharing.get_untracked() {
            stop_sharing(&conn, set_is_sharing, own_preview_hidden, expanded, my_peer_id);
            return;
        }

        let conn = conn.clone();
        let my_peer_id_value = my_peer_id.get_untracked();
        set_status.set("Selecione a tela para compartilhar...".to_string());

        spawn_local(async move {
            let stream = match capture_display().await {
                Ok(stream) => stream,
                Err(_) => {
                    set_status.set("Conectado.".to_string());
                    return;
                }
            };

            if let Some(peer_id) = my_peer_id_value.as_deref() {
                if let Some(document) = web_sys::window().and_then(|w| w.document()) {
                    if let Some(video) = document.get_element_by_id(&format!("video-self-{peer_id}")) {
                        let video: web_sys::HtmlVideoElement = video.unchecked_into();
                        video.set_src_object(Some(&stream));
                        let _ = video.play();
                    }
                }
            }
            *conn.local_stream.borrow_mut() = Some(stream.clone());
            set_is_sharing.set(true);

            // O botão nativo "Stop sharing" do navegador dispara `onended`
            // direto na track, sem passar pelo nosso `toggle_share`.
            if let Ok(track) = stream.get_tracks().get(0).dyn_into::<MediaStreamTrack>() {
                let conn_for_end = conn.clone();
                let onended = wasm_bindgen::prelude::Closure::<dyn FnMut()>::new(move || {
                    stop_sharing(&conn_for_end, set_is_sharing, own_preview_hidden, expanded, my_peer_id);
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
fn stop_sharing(
    conn: &RoomConnection,
    set_is_sharing: WriteSignal<bool>,
    own_preview_hidden: RwSignal<bool>,
    expanded: RwSignal<Option<String>>,
    my_peer_id: ReadSignal<Option<String>>,
) {
    use wasm_bindgen::JsCast;

    if let Some(stream) = conn.local_stream.borrow_mut().take() {
        for track in stream.get_tracks().iter() {
            let track: web_sys::MediaStreamTrack = track.unchecked_into();
            track.stop();
        }
    }
    // O <video> de preview continua com `srcObject` apontando pra essa stream
    // mesmo depois das tracks paradas — em vários Chromium isso basta pro
    // indicador nativo de compartilhamento (bolinha vermelha na aba + barra
    // "parar de compartilhar") continuar visível. Limpar o `srcObject`
    // remove a última referência e libera o indicador de verdade.
    if let Some(peer_id) = my_peer_id.get_untracked() {
        if let Some(document) = web_sys::window().and_then(|w| w.document()) {
            if let Some(video) = document.get_element_by_id(&format!("video-self-{peer_id}")) {
                let video: web_sys::HtmlVideoElement = video.unchecked_into();
                video.set_src_object(None);
            }
        }
    }
    for (_, pc) in conn.outgoing.borrow_mut().drain() {
        pc.close();
    }
    if let Some(ws) = conn.ws.borrow().as_ref() {
        ws.send(&crate::signaling::protocol::ClientMessage::StopShare);
    }
    set_is_sharing.set(false);
    own_preview_hidden.set(false);
    // O servidor só manda `PeerStoppedSharing` pros outros membros, então
    // ninguém encolhe o foco por nós — sem isso a grade travava em
    // `grid--focused` com um card vazio.
    expanded.update(|current| {
        if *current == my_peer_id.get_untracked() {
            *current = None;
        }
    });
}

#[cfg(not(feature = "hydrate"))]
fn invite_click_handler(_room_code: String, _invite_copied: RwSignal<bool>) -> impl Fn(leptos::ev::MouseEvent) + Clone + 'static {
    move |_| {}
}

/// `invite_copied` liga por 2s só pra confirmar visualmente a cópia — a
/// Clipboard API não avisa sozinha.
#[cfg(feature = "hydrate")]
fn invite_click_handler(room_code: String, invite_copied: RwSignal<bool>) -> impl Fn(leptos::ev::MouseEvent) + Clone + 'static {
    use leptos::task::spawn_local;
    use wasm_bindgen::JsCast;
    use wasm_bindgen_futures::JsFuture;

    move |_| {
        let Some(window) = web_sys::window() else { return };
        let origin = window.location().origin().unwrap_or_default();
        let link = format!("{origin}/r/{room_code}");
        let promise = window.navigator().clipboard().write_text(&link);

        spawn_local(async move {
            if JsFuture::from(promise).await.is_err() {
                return;
            }
            invite_copied.set(true);
            let Some(window) = web_sys::window() else { return };
            let reset = wasm_bindgen::prelude::Closure::once_into_js(move || invite_copied.set(false));
            let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(reset.as_ref().unchecked_ref(), 2000);
        });
    }
}

#[cfg(not(feature = "hydrate"))]
fn leave_or_stop_watching_handler(
    _conn: RoomConnection,
    _watching: RwSignal<std::collections::HashSet<String>>,
    _expanded: RwSignal<Option<String>>,
    _my_peer_id: ReadSignal<Option<String>>,
) -> impl Fn(leptos::ev::MouseEvent) + Clone + 'static {
    move |_| {}
}

#[cfg(feature = "hydrate")]
fn leave_or_stop_watching_handler(
    conn: RoomConnection,
    watching: RwSignal<std::collections::HashSet<String>>,
    expanded: RwSignal<Option<String>>,
    my_peer_id: ReadSignal<Option<String>>,
) -> impl Fn(leptos::ev::MouseEvent) + Clone + 'static {
    use leptos_router::hooks::use_navigate;

    use crate::signaling::protocol::ClientMessage;

    move |_| {
        let Some(focused_peer_id) = expanded.get_untracked() else {
            if let Some(ws) = conn.ws.borrow().as_ref() {
                ws.close();
            }
            let navigate = use_navigate();
            navigate("/", Default::default());
            return;
        };

        expanded.set(None);
        if my_peer_id.get_untracked().as_deref() == Some(focused_peer_id.as_str()) {
            return;
        }

        watching.update(|w| {
            w.remove(&focused_peer_id);
        });
        if let Some(pc) = conn.incoming.borrow_mut().remove(&focused_peer_id) {
            pc.close();
        }
        if let Some(ws) = conn.ws.borrow().as_ref() {
            ws.send(&ClientMessage::StopWatching { sharer_id: focused_peer_id });
        }
    }
}

#[cfg(not(feature = "hydrate"))]
fn setup_auto_hide_controls(
    _controls_visible: RwSignal<bool>,
) -> (impl Fn() + Clone + 'static, impl Fn() + Clone + 'static) {
    (|| {}, || {})
}

#[cfg(feature = "hydrate")]
fn setup_auto_hide_controls(
    controls_visible: RwSignal<bool>,
) -> (impl Fn() + Clone + 'static, impl Fn() + Clone + 'static) {
    use std::cell::Cell;
    use std::rc::Rc;

    use wasm_bindgen::prelude::Closure;
    use wasm_bindgen::JsCast;

    const HIDE_AFTER_MS: i32 = 3000;

    let window = web_sys::window().expect("função hydrate roda dentro de um navegador");
    let timeout_id: Rc<Cell<Option<i32>>> = Rc::new(Cell::new(None));

    let cancel_pending = {
        let window = window.clone();
        let timeout_id = timeout_id.clone();
        move || {
            if let Some(id) = timeout_id.take() {
                window.clear_timeout_with_handle(id);
            }
        }
    };

    let schedule_hide = {
        let window = window.clone();
        let timeout_id = timeout_id.clone();
        let cancel_pending = cancel_pending.clone();
        move || {
            cancel_pending();
            let hide = Closure::once_into_js(move || controls_visible.set(false));
            if let Ok(id) = window
                .set_timeout_with_callback_and_timeout_and_arguments_0(hide.as_ref().unchecked_ref(), HIDE_AFTER_MS)
            {
                timeout_id.set(Some(id));
            }
        }
    };

    let show_and_schedule_hide = {
        let schedule_hide = schedule_hide.clone();
        move || {
            controls_visible.set(true);
            schedule_hide();
        }
    };

    let on_move = {
        let show_and_schedule_hide = show_and_schedule_hide.clone();
        Closure::<dyn FnMut()>::new(move || show_and_schedule_hide())
    };
    let _ = window.add_event_listener_with_callback("mousemove", on_move.as_ref().unchecked_ref());
    on_move.forget();

    show_and_schedule_hide();

    (cancel_pending, schedule_hide)
}

#[cfg(not(feature = "hydrate"))]
fn setup_adaptive_grid(
    _members: ReadSignal<Vec<RoomMember>>,
    _hide_idle: RwSignal<bool>,
    _own_preview_hidden: RwSignal<bool>,
    _is_sharing: ReadSignal<bool>,
    _expanded: RwSignal<Option<String>>,
) {
}

/// Igual ao Discord: quantas colunas/linhas o grid usa depende de quantos
/// cards estão visíveis *e* do formato do próprio container — 1 pessoa
/// ocupa a tela inteira, 2 ficam lado a lado (ou empilhadas, se a janela for
/// mais alta que larga), e por aí vai, ao contrário de um `grid-template-
/// columns` fixo que deixaria cards minúsculos boiando num container gigante
/// quando há poucos membros. Como esse cálculo depende do tamanho real do
/// container em pixels (não só da contagem), não dá pra fazer só com CSS —
/// por isso o `grid-template-columns/rows` é escrito via `style` inline
/// depois de medir o container. `#[cfg(feature = "hydrate")]` já filtra
/// então essa função só roda com um navegador de verdade disponível.
#[cfg(feature = "hydrate")]
fn setup_adaptive_grid(
    members: ReadSignal<Vec<RoomMember>>,
    hide_idle: RwSignal<bool>,
    own_preview_hidden: RwSignal<bool>,
    is_sharing: ReadSignal<bool>,
    expanded: RwSignal<Option<String>>,
) {
    use wasm_bindgen::prelude::Closure;
    use wasm_bindgen::JsCast;

    let schedule_recompute = move || {
        let raf_callback = Closure::once_into_js(move || recompute_adaptive_grid(expanded));
        if let Some(window) = web_sys::window() {
            let _ = window.request_animation_frame(raf_callback.as_ref().unchecked_ref());
        }
    };

    // Reagir a qualquer sinal que muda quantos cards ficam visíveis (entrar/
    // sair da sala, ocultar quem não transmite, esconder o próprio preview,
    // começar/parar de compartilhar, entrar/sair do modo foco). A recontagem
    // em si acontece no próximo frame (`schedule_recompute`), depois que o
    // DOM já refletiu as classes `hidden` novas.
    Effect::new(move |_| {
        members.track();
        hide_idle.track();
        own_preview_hidden.track();
        is_sharing.track();
        expanded.track();
        schedule_recompute();
    });

    let on_resize = Closure::<dyn FnMut()>::new(move || schedule_recompute());
    if let Some(window) = web_sys::window() {
        let _ = window.add_event_listener_with_callback("resize", on_resize.as_ref().unchecked_ref());
    }
    on_resize.forget();
}

#[cfg(feature = "hydrate")]
fn recompute_adaptive_grid(expanded: RwSignal<Option<String>>) {
    use wasm_bindgen::JsCast;

    let Some(document) = web_sys::window().and_then(|w| w.document()) else { return };
    let Some(grid) = document.get_element_by_id("member-grid") else { return };
    let grid: web_sys::HtmlElement = grid.unchecked_into();
    let style = grid.style();

    if expanded.get_untracked().is_some() {
        // `.grid--focused` já define seu próprio grid-template via CSS —
        // limpa qualquer valor inline deixado pelo modo normal, senão o
        // inline (que tem prioridade sobre a regra da classe) trava o
        // layout do modo foco.
        let _ = style.remove_property("grid-template-columns");
        let _ = style.remove_property("grid-template-rows");
        return;
    }

    let Ok(list) = grid.query_selector_all(".card:not(.hidden)") else { return };
    let visible = list.length() as usize;
    if visible == 0 {
        return;
    }

    let width = grid.client_width() as f64;
    let height = grid.client_height() as f64;
    if width <= 0.0 || height <= 0.0 {
        return;
    }

    // Fórmula clássica de grid de videochamada: escolhe o número de colunas
    // que deixa cada célula o mais próxima possível de um retângulo 16:9,
    // dado o formato real do container — é por isso que 2 pessoas ficam
    // lado a lado numa janela larga mas empilhadas numa janela estreita.
    const TILE_ASPECT: f64 = 16.0 / 9.0;
    let container_ratio = width / height;
    let raw_cols = ((visible as f64) * container_ratio / TILE_ASPECT).sqrt();
    let cols = (raw_cols.round().max(1.0) as usize).min(visible);
    let rows = visible.div_ceil(cols);

    let _ = style.set_property("grid-template-columns", &format!("repeat({cols}, minmax(0, 1fr))"));
    let _ = style.set_property("grid-template-rows", &format!("repeat({rows}, minmax(0, 1fr))"));
}
