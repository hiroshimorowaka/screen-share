mod connection;
#[cfg(debug_assertions)]
mod dev_preview;
mod grid;
mod invite;
mod latency;
mod media_controls;
mod member_card;
mod message_handler;
mod quality;
mod room_check;
mod share;
mod watch;

#[cfg(debug_assertions)]
pub(crate) use dev_preview::DevRoomPreviewPage;

use leptos::prelude::*;
use leptos_router::hooks::use_params_map;

use connection::{adopt_pending_session, setup_room_connection, RoomConnection, RoomSignals};
use grid::{setup_adaptive_grid, setup_auto_hide_controls};
#[cfg(feature = "hydrate")]
use invite::build_invite_link;
use invite::invite_click_handler;
use latency::setup_ping_loop;
use media_controls::setup_fullscreen_autohide_controls;
use member_card::{member_cards, MemberCardSignals};
use room_check::start_room_check;
#[cfg(feature = "hydrate")]
use share::start_sharing;
use share::{share_supported, share_toggle_handler};
use watch::leave_or_stop_watching_handler;
#[cfg(feature = "hydrate")]
use watch::leave_room;

use crate::ui::components::color_picker::ColorPicker;
use crate::ui::components::icons::{
    icon_check, icon_eye_off, icon_link, icon_log_out, icon_monitor, icon_screen_off,
    icon_video_off,
};
use crate::ui::components::status::status_meta;
use crate::ui::components::status_message::StatusMessage;
use screen_share_protocol::MAX_MEMBERS;

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
    let (status, set_status) = signal("Informe o nick da sala.".to_string());
    let (authenticated, set_authenticated) = signal(false);
    let (room_exists, set_room_exists) = signal(None::<bool>);
    let (room_name, set_room_name) = signal(None::<String>);
    // Assume a password may be required until the room check resolves — the
    // join panel that reads this stays hidden the whole time anyway (see
    // `room_exists` above), so there's no flash of the wrong state.
    let (requires_password, set_requires_password) = signal(true);
    let (members, set_members) = signal(Vec::<RoomMember>::new());
    let (my_peer_id, set_my_peer_id) = signal(None::<String>);
    let (is_sharing, set_is_sharing) = signal(false);
    let connection_errors = RwSignal::new(std::collections::HashSet::<String>::new());
    let watching = RwSignal::new(std::collections::HashSet::<String>::new());
    let expanded = RwSignal::new(None::<String>);
    let watchers_by_sharer = RwSignal::new(std::collections::HashMap::<String, Vec<String>>::new());
    let latency_by_peer = RwSignal::new(std::collections::HashMap::<String, u32>::new());
    let turn_credentials = RwSignal::new(None::<screen_share_protocol::TurnCredentials>);
    let own_preview_hidden = RwSignal::new(false);
    let volume_by_peer = RwSignal::new(std::collections::HashMap::<String, f64>::new());
    let muted_by_peer = RwSignal::new(std::collections::HashSet::<String>::new());
    let quality_by_peer = RwSignal::new(std::collections::HashMap::<
        String,
        screen_share_protocol::QualityLevel,
    >::new());
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
        latency_by_peer,
        turn_credentials,
    };

    let join_room = setup_room_connection(initial_code.clone(), conn.clone(), room_signals);

    adopt_pending_session(
        initial_code.clone(),
        conn.clone(),
        room_signals,
        set_requires_password,
    );

    // Reloading the page while still in a room shouldn't drop back to the
    // nick/password gate — rejoin silently with whatever this same tab used
    // last time, same as `adopt_pending_session` does for the creator's own
    // first load. Only runs if that didn't already authenticate us.
    #[cfg(feature = "hydrate")]
    if !authenticated.get_untracked() {
        if let Some(stored) = crate::ui::client::storage::load_room_session(&initial_code) {
            join_room(stored.nick, stored.color, stored.password);
        }
    }

    // The desktop tray's quick-share flow: once the room-creation join
    // above authenticates, start sharing immediately with no click, then
    // hand the invite link to the desktop shell as soon as the share goes
    // live. Each effect has its own "already done" latch — `authenticated`
    // and `is_sharing` can each change more than once over the page's
    // life, but this must only ever fire once.
    #[cfg(feature = "hydrate")]
    {
        let quick_share_active = crate::ui::quick_share::requested();
        let auto_share_started = RwSignal::new(false);
        let auto_share_notified = RwSignal::new(false);
        let room_code_for_notify = initial_code.clone();
        let room_code_for_cancel = initial_code.clone();
        let conn_for_auto_share = conn.clone();
        let conn_for_cancel = conn.clone();

        Effect::new(move |_| {
            if quick_share_active && authenticated.get() && !auto_share_started.get_untracked() {
                auto_share_started.set(true);
                // Nobody's watching this hidden window to pick a screen a
                // second time — cancelling the picker here means leaving,
                // not sitting in the room unshared forever.
                let conn_for_cancel = conn_for_cancel.clone();
                let room_code_for_cancel = room_code_for_cancel.clone();
                start_sharing(
                    conn_for_auto_share.clone(),
                    set_is_sharing,
                    own_preview_hidden,
                    set_status,
                    my_peer_id,
                    expanded,
                    move || leave_room(&conn_for_cancel, &room_code_for_cancel),
                );
            }
        });

        Effect::new(move |_| {
            if quick_share_active && is_sharing.get() && !auto_share_notified.get_untracked() {
                auto_share_notified.set(true);
                if let Some(link) = build_invite_link(&room_code_for_notify) {
                    crate::ui::client::webrtc::notify_desktop_share_ready(&link);
                }
            }
        });
    }

    start_room_check(
        initial_code.clone(),
        authenticated,
        set_room_exists,
        set_room_name,
        set_requires_password,
    );

    let manual_join = {
        let join_room = join_room.clone();
        // Only read from `#[cfg(feature = "hydrate")]` code below — an
        // `ssr`-only compile sees no reads and would otherwise flag it.
        #[cfg_attr(not(feature = "hydrate"), allow(unused_variables))]
        let room_code = initial_code.clone();
        move |ev: leptos::ev::SubmitEvent| {
            ev.prevent_default();
            let nick_value = nick.get_untracked().trim().to_string();
            let password_value = password.get_untracked();
            if nick_value.is_empty()
                || (requires_password.get_untracked() && password_value.is_empty())
            {
                set_status.set("Preencha nick e senha.".to_string());
                return;
            }
            let password_value = (!password_value.is_empty()).then_some(password_value);
            #[cfg(feature = "hydrate")]
            crate::ui::client::storage::save_room_session(
                &room_code,
                &crate::ui::client::storage::RoomSession {
                    nick: nick_value.clone(),
                    color: color.get_untracked(),
                    password: password_value.clone(),
                },
            );
            join_room(nick_value, color.get_untracked(), password_value);
        }
    };

    let toggle_share = share_toggle_handler(
        conn.clone(),
        is_sharing,
        set_is_sharing,
        own_preview_hidden,
        set_status,
        my_peer_id,
        expanded,
    );
    let invite_click = invite_click_handler(initial_code.clone(), invite_copied);
    let leave_or_stop_watching = leave_or_stop_watching_handler(
        conn.clone(),
        watching,
        expanded,
        my_peer_id,
        initial_code.clone(),
    );
    let (pause_hide_controls, resume_hide_controls) = setup_auto_hide_controls(controls_visible);
    setup_adaptive_grid(members, hide_idle, own_preview_hidden, is_sharing, expanded);
    setup_fullscreen_autohide_controls();
    setup_ping_loop(conn.clone());

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
                <label class="field" class:hidden=move || !requires_password.get()>
                    <span class="field__label">"Senha da sala"</span>
                    <input class="field__input" type="password" prop:value=password
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
                    volume_by_peer,
                    muted_by_peer,
                    latency_by_peer,
                    quality_by_peer,
                })}
            </div>
            <div
                class="room-controls"
                class:room-controls--hidden=move || !controls_visible.get()
                on:mouseenter=move |_| pause_hide_controls()
                on:mouseleave=move |_| resume_hide_controls()
            >
                <div class="control-group">
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
                        {icon_video_off}
                    </button>
                </div>
                <div class="control-group control-group--danger">
                    <button
                        class="icon-btn icon-btn--danger"
                        title=move || if expanded.get().is_some() { "Parar de assistir" } else { "Sair da sala" }
                        aria-label=move || if expanded.get().is_some() { "Parar de assistir" } else { "Sair da sala" }
                        on:click=leave_or_stop_watching
                    >
                        {move || if expanded.get().is_some() { icon_screen_off().into_any() } else { icon_log_out().into_any() }}
                    </button>
                </div>
            </div>
        </div>
    }
}
