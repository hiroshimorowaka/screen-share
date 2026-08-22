mod connection;
mod grid;
mod invite;
mod media_controls;
mod member_card;
mod message_handler;
mod room_check;
mod share;
mod watch;

use leptos::prelude::*;
use leptos_router::hooks::use_params_map;

use connection::{adopt_pending_session, setup_room_connection, RoomConnection, RoomSignals};
use grid::{setup_adaptive_grid, setup_auto_hide_controls};
use invite::invite_click_handler;
use member_card::{member_cards, MemberCardSignals};
use room_check::start_room_check;
use share::{share_supported, share_toggle_handler};
use watch::leave_or_stop_watching_handler;

use crate::signaling::protocol::MAX_MEMBERS;
use crate::ui::components::color_picker::ColorPicker;
use crate::ui::components::icons::{icon_check, icon_eye_off, icon_link, icon_log_out, icon_monitor, icon_screen_off, icon_video, icon_video_off};
use crate::ui::components::status::status_meta;
use crate::ui::components::status_message::StatusMessage;

#[derive(Clone, PartialEq)]
pub struct RoomMember {
    pub peer_id: String,
    pub nick: String,
    pub color: String,
    pub sharing: bool,
}

#[component]
pub fn RoomPage() -> impl IntoView {
    let params = use_params_map();
    let code = move || params.read().get("code").unwrap_or_default();
    let initial_code = params.read_untracked().get("code").unwrap_or_default();

    // Starts at the SSR default value; the real one (localStorage) only
    // arrives after mount, or hydration of the selected color swatch breaks.
    let (nick, set_nick) = signal(String::new());
    let (color, set_color) = signal(crate::ui::components::palette::DEFAULT_COLOR.to_string());
    crate::ui::profile::load_profile_after_mount(set_nick, set_color);
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
    let room_signals = RoomSignals {
        set_status,
        set_authenticated,
        set_room_name,
        set_members,
        set_my_peer_id,
        my_peer_id,
        set_room_exists,
        watching,
        expanded,
        watchers_by_sharer,
        connection_errors,
    };

    let join_room = setup_room_connection(initial_code.clone(), conn.clone(), room_signals);

    adopt_pending_session(initial_code.clone(), conn.clone(), room_signals);

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
        // class:hidden instead of `<Show>`: Leptos 0.8 requires Send + Sync
        // on `<Show>` children, and the form captures an
        // `Rc<RefCell<WsClient>>`, which is not.
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
                {member_cards(conn, MemberCardSignals {
                    members,
                    my_peer_id,
                    is_sharing,
                    watching,
                    expanded,
                    watchers_by_sharer,
                    own_preview_hidden,
                    hide_idle,
                    connection_errors,
                })}
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
