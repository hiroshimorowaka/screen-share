use std::collections::HashMap;

use leptos::prelude::*;

use crate::ui::pages::palette::{color_hex, palette_ids};
use crate::ui::pages::status::status_meta;
use crate::signaling::protocol::MAX_MEMBERS;

#[component]
pub fn HomePage() -> impl IntoView {
    // Sinais começam no valor que o SSR usaria (vazio/padrão); o valor real
    // do localStorage só é aplicado depois do mount, ou a hidratação do
    // Leptos quebra (bindings de classe reagem errado, e o <For> de
    // `recent_rooms` diverge em tamanho do que o servidor renderizou).
    let (nick, set_nick) = signal(String::new());
    let (color, set_color) = signal(crate::ui::pages::palette::DEFAULT_COLOR.to_string());
    load_profile_after_mount(set_nick, set_color);
    let (room_name, set_room_name) = signal(String::new());
    load_last_room_name_after_mount(set_room_name);
    let (password, set_password) = signal(String::new());
    let (status, set_status) = signal("Pronto para criar uma sala.".to_string());
    let (submitting, set_submitting) = signal(false);
    let (recent_rooms, set_recent_rooms) = signal(Vec::<crate::ui::profile::RecentRoom>::new());
    // Contagem de membros por sala: diferente de `recent_rooms`, sempre vem
    // do servidor — muda a cada entrada/saída, não persiste no navegador.
    let (member_counts, set_member_counts) = signal(HashMap::<String, usize>::new());

    load_recent_rooms_after_mount(set_recent_rooms);
    prune_recent_rooms(set_recent_rooms, set_member_counts);

    let create_room = create_room_handler(nick, color, room_name, password, set_status, set_submitting);

    let (join_input, set_join_input) = signal(String::new());
    let (join_status, set_join_status) = signal(String::new());
    let join_room = join_room_handler(join_input, set_join_status);

    view! {
        <div class="home-layout">
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
                    {
                        let code_for_hidden = room.code.clone();
                        let code_for_count = room.code.clone();
                        view! {
                            <a class="recent-room" href=format!("/r/{}", room.code)>
                                <span class="recent-room__name">{room.name.clone()}</span>
                                <div class="recent-room__meta">
                                    <span class="recent-room__code">{room.code.clone()}</span>
                                    <span
                                        class="room-member-count"
                                        class:hidden=move || !member_counts.get().contains_key(&code_for_hidden)
                                    >
                                        {move || {
                                            member_counts.get().get(&code_for_count).map(|count| format!("{count}/{MAX_MEMBERS}")).unwrap_or_default()
                                        }}
                                    </span>
                                </div>
                            </a>
                        }
                    }
                </For>
            </div>
        </div>

        <div class="panel">
            <h1>"Entrar em uma sala"</h1>
            <p class="subtext">"Cole o código da sala ou o link completo do convite — você poderá informar o nick e a senha na página da sala."</p>

            <form on:submit=join_room>
                <label class="field">
                    <span class="field__label">"Código ou link da sala"</span>
                    <input
                        class="field__input"
                        type="text"
                        required
                        prop:value=join_input
                        on:input:target=move |ev| set_join_input.set(ev.target().value())
                    />
                </label>
                <button class="btn btn--primary" type="submit">"Entrar na sala"</button>
            </form>

            <p class="status-text status-text--error" class:hidden=move || join_status.get().is_empty()>
                {join_status}
            </p>
        </div>
        </div>
    }
}

/// Aceita tanto o código bruto quanto o link completo do convite.
/// Normaliza pra maiúsculas — `generate_room_code` só gera códigos
/// maiúsculos, e sem isso colar em minúsculas nunca bateria com a sala real.
///
/// `cfg(any(test, feature = "hydrate"))`, não só `hydrate`: evita o aviso de
/// código morto num build `ssr`-only, mas mantém a função plain Rust
/// (sem `web-sys`) e testável sem navegador.
#[cfg(any(test, feature = "hydrate"))]
fn extract_room_code(input: &str) -> Option<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }
    let after_marker = match trimmed.find("/r/") {
        Some(idx) => &trimmed[idx + "/r/".len()..],
        None => trimmed,
    };
    let code = after_marker.split(['/', '?', '#']).next().unwrap_or("").trim();
    if code.is_empty() {
        None
    } else {
        Some(code.to_ascii_uppercase())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_room_code_accepts_a_bare_code() {
        assert_eq!(extract_room_code("ab3d9f2k"), Some("AB3D9F2K".to_string()));
    }

    #[test]
    fn extract_room_code_accepts_a_full_link() {
        assert_eq!(extract_room_code("https://example.com/r/AB3D9F2K"), Some("AB3D9F2K".to_string()));
    }

    #[test]
    fn extract_room_code_strips_trailing_slash_and_query_string() {
        assert_eq!(extract_room_code("https://example.com/r/AB3D9F2K/?x=1"), Some("AB3D9F2K".to_string()));
    }

    #[test]
    fn extract_room_code_trims_surrounding_whitespace() {
        assert_eq!(extract_room_code("  AB3D9F2K  "), Some("AB3D9F2K".to_string()));
    }

    #[test]
    fn extract_room_code_rejects_blank_input() {
        assert_eq!(extract_room_code("   "), None);
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
fn load_last_room_name_after_mount(_set_room_name: WriteSignal<String>) {}

#[cfg(feature = "hydrate")]
fn load_last_room_name_after_mount(set_room_name: WriteSignal<String>) {
    use leptos::task::spawn_local;

    spawn_local(async move {
        if let Some(name) = crate::ui::client::storage::load_last_room_name() {
            set_room_name.set(name);
        }
    });
}

#[cfg(not(feature = "hydrate"))]
fn load_recent_rooms_after_mount(_set_recent_rooms: WriteSignal<Vec<crate::ui::profile::RecentRoom>>) {}

#[cfg(feature = "hydrate")]
fn load_recent_rooms_after_mount(set_recent_rooms: WriteSignal<Vec<crate::ui::profile::RecentRoom>>) {
    use leptos::task::spawn_local;

    spawn_local(async move {
        set_recent_rooms.set(crate::ui::client::storage::load_recent_rooms());
    });
}

#[cfg(not(feature = "hydrate"))]
fn prune_recent_rooms(
    _set_recent_rooms: WriteSignal<Vec<crate::ui::profile::RecentRoom>>,
    _set_member_counts: WriteSignal<HashMap<String, usize>>,
) {}

#[cfg(feature = "hydrate")]
fn prune_recent_rooms(
    set_recent_rooms: WriteSignal<Vec<crate::ui::profile::RecentRoom>>,
    set_member_counts: WriteSignal<HashMap<String, usize>>,
) {
    use leptos::task::spawn_local;

    use crate::ui::client::{rooms_api::check_room, storage::remove_recent_room};

    for room in crate::ui::client::storage::load_recent_rooms() {
        let code = room.code.clone();
        spawn_local(async move {
            if let Some(status) = check_room(&code).await {
                if status.exists {
                    if let Some(count) = status.member_count {
                        set_member_counts.update(|counts| {
                            counts.insert(code.clone(), count);
                        });
                    }
                } else {
                    remove_recent_room(&code);
                    set_recent_rooms.update(|rooms| rooms.retain(|r| r.code != code));
                }
            }
        });
    }
}

#[cfg(not(feature = "hydrate"))]
fn join_room_handler(_join_input: ReadSignal<String>, _set_join_status: WriteSignal<String>) -> impl Fn(leptos::ev::SubmitEvent) + 'static {
    move |ev: leptos::ev::SubmitEvent| ev.prevent_default()
}

/// Só resolve o código e navega pra `/r/{code}` — nick, cor e senha ficam
/// pro portão de entrada da própria página da sala.
#[cfg(feature = "hydrate")]
fn join_room_handler(join_input: ReadSignal<String>, set_join_status: WriteSignal<String>) -> impl Fn(leptos::ev::SubmitEvent) + 'static {
    use leptos_router::hooks::use_navigate;

    move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();

        let Some(code) = extract_room_code(&join_input.get_untracked()) else {
            set_join_status.set("Informe o código da sala ou o link completo do convite.".to_string());
            return;
        };

        let navigate = use_navigate();
        navigate(&format!("/r/{code}"), Default::default());
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

    use crate::ui::client::session::{self, PendingSession};
    use crate::ui::client::socket::WsClient;
    use crate::ui::client::storage::{ensure_device_id, save_last_room_name, save_profile, save_recent_room};
    use crate::ui::profile::{Profile, RecentRoom};
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
                if let ServerMessage::Joined { peer_id, room, room_name, members, active_sharers, .. } = msg {
                    save_profile(&Profile { nick: nick_value.clone(), color: color_value.clone() });
                    save_recent_room(RecentRoom { code: room.clone(), name: room_name.clone() });
                    save_last_room_name(&room_name);
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
                                device_id: ensure_device_id(),
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
