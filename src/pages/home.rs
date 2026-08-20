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
