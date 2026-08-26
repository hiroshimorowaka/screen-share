use leptos::prelude::*;

#[cfg(not(feature = "hydrate"))]
pub fn load_last_room_name_after_mount(_set_room_name: WriteSignal<String>) {}

#[cfg(feature = "hydrate")]
pub fn load_last_room_name_after_mount(set_room_name: WriteSignal<String>) {
    use leptos::task::spawn_local;

    spawn_local(async move {
        if let Some(name) = crate::ui::client::storage::load_last_room_name() {
            set_room_name.set(name);
        }
    });
}

#[cfg(not(feature = "hydrate"))]
pub fn create_room_handler(
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
pub fn create_room_handler(
    nick: ReadSignal<String>,
    color: ReadSignal<String>,
    room_name: ReadSignal<String>,
    password: ReadSignal<String>,
    set_status: WriteSignal<String>,
    set_submitting: WriteSignal<bool>,
) -> impl Fn(leptos::ev::SubmitEvent) + 'static {
    move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();

        let nick_value = nick.get_untracked().trim().to_string();
        let color_value = color.get_untracked();
        let room_name_value = room_name.get_untracked().trim().to_string();
        let password_value = password.get_untracked();
        if nick_value.is_empty() || room_name_value.is_empty() {
            set_status.set("Preencha o nick e o nome da sala.".to_string());
            return;
        }
        let password_value = (!password_value.is_empty()).then_some(password_value);

        submit_create_room(nick_value, color_value, room_name_value, password_value, set_status, set_submitting, false);
    }
}

/// Connects, creates the room, and — once joined — navigates to it. Shared
/// by the real form (`create_room_handler`) and the desktop tray's
/// quick-share flow (`start_quick_share_after_mount`), which supplies its
/// own generated nick/room name instead of reading them from form signals.
/// `quick_share` only changes which URL the post-join navigation lands on —
/// see `crate::ui::quick_share::room_path_with_flag`.
#[cfg(feature = "hydrate")]
pub fn submit_create_room(
    nick_value: String,
    color_value: String,
    room_name_value: String,
    password_value: Option<String>,
    set_status: WriteSignal<String>,
    set_submitting: WriteSignal<bool>,
    quick_share: bool,
) {
    use std::cell::RefCell;
    use std::rc::Rc;

    use leptos_router::hooks::use_navigate;

    use crate::signaling::protocol::{ClientMessage, ServerMessage};
    use crate::ui::client::session::{self, PendingSession};
    use crate::ui::client::socket::WsClient;
    use crate::ui::client::storage::{
        ensure_device_id, save_last_room_name, save_profile, save_recent_room, save_room_session, RoomSession,
    };
    use crate::ui::profile::{Profile, RecentRoom};

    set_submitting.set(true);
    set_status.set("Criando sala...".to_string());

    let ws_slot: Rc<RefCell<Option<WsClient>>> = Rc::new(RefCell::new(None));
    let navigate = use_navigate();

    let requires_password = password_value.is_some();
    let on_message = {
        let ws_slot = ws_slot.clone();
        let nick_value = nick_value.clone();
        let color_value = color_value.clone();
        let password_value = password_value.clone();
        move |msg: ServerMessage| {
            if let ServerMessage::Joined { peer_id, room, room_name, members, active_sharers, .. } = msg {
                save_profile(&Profile { nick: nick_value.clone(), color: color_value.clone() });
                save_recent_room(RecentRoom { code: room.clone(), name: room_name.clone() });
                save_last_room_name(&room_name);
                save_room_session(
                    &room,
                    &RoomSession { nick: nick_value.clone(), color: color_value.clone(), password: password_value.clone() },
                );
                if let Some(ws) = ws_slot.borrow_mut().take() {
                    session::stash(PendingSession {
                        room: room.clone(),
                        room_name,
                        ws,
                        peer_id,
                        members,
                        active_sharers,
                        requires_password,
                    });
                }
                let target = if quick_share {
                    crate::ui::quick_share::room_path_with_flag(&room)
                } else {
                    format!("/r/{room}")
                };
                navigate(&target, Default::default());
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

/// Kicks off the desktop tray's "share now" flow when the URL carries the
/// `quick_share` flag: a random nick (unless the browser already has a
/// saved profile) and a random room name, created and joined with no form
/// interaction at all. Deferred to after mount, like the other
/// `_after_mount` helpers here — `set_status`/`set_submitting` feed
/// rendered output, so mutating them during the initial hydration pass
/// risks a hydration mismatch.
#[cfg(not(feature = "hydrate"))]
pub fn start_quick_share_after_mount(_set_status: WriteSignal<String>, _set_submitting: WriteSignal<bool>) {}

#[cfg(feature = "hydrate")]
pub fn start_quick_share_after_mount(set_status: WriteSignal<String>, set_submitting: WriteSignal<bool>) {
    use leptos::task::spawn_local;

    if !crate::ui::quick_share::requested() {
        return;
    }

    spawn_local(async move {
        let profile = crate::ui::client::storage::load_profile();
        let nick_value = if profile.nick.trim().is_empty() { crate::ui::quick_share::random_nick() } else { profile.nick };
        let room_name_value = crate::ui::quick_share::random_room_name();
        submit_create_room(nick_value, profile.color, room_name_value, None, set_status, set_submitting, true);
    });
}
