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

fn initial_profile() -> crate::profile::Profile {
    initial_profile_impl()
}

#[cfg(not(feature = "hydrate"))]
fn initial_profile_impl() -> crate::profile::Profile {
    crate::profile::Profile::default()
}

#[cfg(feature = "hydrate")]
fn initial_profile_impl() -> crate::profile::Profile {
    crate::client::storage::load_profile()
}

fn initial_recent_rooms() -> Vec<crate::profile::RecentRoom> {
    initial_recent_rooms_impl()
}

#[cfg(not(feature = "hydrate"))]
fn initial_recent_rooms_impl() -> Vec<crate::profile::RecentRoom> {
    Vec::new()
}

#[cfg(feature = "hydrate")]
fn initial_recent_rooms_impl() -> Vec<crate::profile::RecentRoom> {
    crate::client::storage::load_recent_rooms()
}

#[cfg(not(feature = "hydrate"))]
fn prune_recent_rooms(_set_recent_rooms: WriteSignal<Vec<crate::profile::RecentRoom>>) {}

#[cfg(feature = "hydrate")]
fn prune_recent_rooms(set_recent_rooms: WriteSignal<Vec<crate::profile::RecentRoom>>) {
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
    use crate::client::storage::{save_profile, save_recent_room};
    use crate::profile::{Profile, RecentRoom};
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
