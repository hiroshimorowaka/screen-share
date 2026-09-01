#[cfg(debug_assertions)]
mod dev_preview;
mod grid;
mod invite;
pub(crate) mod media_controls;
mod member_card;
mod room_check;
mod touch;
mod watch;

#[cfg(debug_assertions)]
pub(crate) use dev_preview::DevRoomPreviewPage;

use leptos::prelude::*;
use leptos_router::hooks::use_params_map;

use crate::session::latency::setup_ping_loop;
#[cfg(feature = "hydrate")]
use crate::session::media::start_sharing;
use crate::session::media::{
    share_supported, share_toggle_handler, sharing_can_have_audio, switch_source_handler,
};
use grid::{setup_adaptive_grid, setup_auto_hide_controls};
#[cfg(feature = "hydrate")]
use invite::build_invite_link;
use invite::invite_click_handler;
use media_controls::setup_fullscreen_autohide_controls;
use member_card::{member_cards, MemberCardSignals};
use room_check::start_room_check;
use watch::leave_or_stop_watching_handler;
#[cfg(feature = "hydrate")]
use watch::leave_room;

use crate::components::color_picker::ColorPicker;
use crate::components::icons::{
    icon_check, icon_eye_off, icon_link, icon_log_out, icon_minimize, icon_monitor,
    icon_screen_off, icon_switch, icon_video_off,
};
use crate::components::status::status_meta;
use crate::components::status_message::StatusMessage;
use crate::components::transmission_menu::TransmissionMenu;
use crate::session::{
    adopt_pending_session, setup_room_connection, RoomMember, RoomSession, RoomSignals,
};
use screen_share_protocol::MAX_MEMBERS;

/// Whether a captured share stream ended up with an audio track — the one
/// signal the web side has for "this share carries audio", since the
/// desktop picker's audio choice never crosses back to the renderer.
#[cfg(feature = "hydrate")]
fn stream_has_audio_track(stream: &web_sys::MediaStream) -> bool {
    use wasm_bindgen::JsCast;

    stream
        .get_tracks()
        .iter()
        .filter_map(|entry| entry.dyn_into::<web_sys::MediaStreamTrack>().ok())
        .any(|track| track.kind() == "audio")
}

#[component]
pub fn RoomPage() -> impl IntoView {
    let params = use_params_map();
    let code = move || params.read().get("code").unwrap_or_default();
    let initial_code = params.read_untracked().get("code").unwrap_or_default();

    // Starts at the SSR default value; the real one (localStorage) only
    // arrives after mount, or hydration of the selected color swatch breaks.
    let (nick, set_nick) = signal(String::new());
    let (color, set_color) = signal(crate::components::palette::DEFAULT_COLOR.to_string());
    crate::features::profile::load_profile_after_mount(set_nick, set_color);
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
    let audio_preset = RwSignal::new(crate::session::audio::AudioPreset::default());
    let video_mode = RwSignal::new(crate::session::video_mode::VideoMode::default());
    // Whether the sharer has silenced their own outgoing audio (the track
    // stays published, viewers just hear silence). Reset to `false` when a
    // share ends — see the effect below.
    let audio_muted = RwSignal::new(false);
    let own_preview_hidden = RwSignal::new(false);
    let volume_by_peer = RwSignal::new(std::collections::HashMap::<String, f64>::new());
    let muted_by_peer = RwSignal::new(std::collections::HashSet::<String>::new());
    let quality_by_peer = RwSignal::new(std::collections::HashMap::<
        String,
        screen_share_protocol::QualityLevel,
    >::new());
    let hide_idle = RwSignal::new(false);
    // Set by the audio self-test once a share of ours has been probed (see
    // the effect below); `None` means "nothing wrong / not checked yet".
    let audio_warning = RwSignal::new(None::<&'static str>);
    // Whether the current share's captured stream actually carries an audio
    // track (see `stream_has_audio_track`). The web side never learns
    // whether the sharer ticked "compartilhar áudio" in the desktop
    // picker, so this is the closest signal for "this share has audio" —
    // it drives the header audio chip's on/off wording. Reset when sharing
    // stops.
    let share_has_audio = RwSignal::new(false);
    let controls_visible = RwSignal::new(true);
    // Touch device? Drives the tap-to-toggle chrome and the touch-only
    // auto-hide behaviour; everything else adapts in CSS. Starts `false`
    // (the SSR assumption) and is corrected on mount by `setup_touch_signal`.
    let (is_touch, set_is_touch) = signal(false);
    touch::setup_touch_signal(set_is_touch);
    let invite_copied = RwSignal::new(false);
    let can_share = share_supported();
    // The desktop shell captures system audio; a plain browser tab can
    // capture its own tab audio through the picker. Either way the
    // audio-quality / mute controls apply — they only stay hidden on a
    // browser that can't screen-share at all.
    let sharing_has_audio = sharing_can_have_audio();

    let conn = RoomSession::new();
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
        audio_preset,
        video_mode,
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
        if let Some(stored) = crate::infra::storage::load_room_session(&initial_code) {
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
        let quick_share_active = crate::quick_share::requested();
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
                    move || leave_room(&conn_for_cancel, &room_code_for_cancel, my_peer_id),
                );
            }
        });

        Effect::new(move |_| {
            if quick_share_active && is_sharing.get() && !auto_share_notified.get_untracked() {
                auto_share_notified.set(true);
                if let Some(link) = build_invite_link(&room_code_for_notify) {
                    crate::infra::webrtc::notify_desktop_share_ready(&link);
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
            crate::infra::storage::save_room_session(
                &room_code,
                &crate::infra::storage::RoomSession {
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
    let set_audio_preset =
        crate::session::audio::set_audio_preset_handler(conn.clone(), audio_preset);
    let set_video_mode =
        crate::session::video_mode::set_video_mode_handler(conn.clone(), video_mode);
    let switch_source = switch_source_handler(
        conn.clone(),
        set_is_sharing,
        own_preview_hidden,
        set_status,
        my_peer_id,
        expanded,
        audio_muted.read_only(),
        video_mode.read_only(),
    );
    let leave_or_stop_watching = leave_or_stop_watching_handler(
        conn.clone(),
        watching,
        expanded,
        my_peer_id,
        initial_code.clone(),
    );
    let (pause_hide_controls, resume_hide_controls) =
        setup_auto_hide_controls(controls_visible, is_touch, expanded);
    setup_adaptive_grid(members, hide_idle, own_preview_hidden, is_sharing, expanded);
    setup_fullscreen_autohide_controls();
    setup_ping_loop(conn.clone());
    // On leaving the room, tear down every peer connection, its callbacks,
    // and the Auto-quality polls — otherwise a leaked callback keeps the
    // whole session alive in memory.
    crate::session::reconnect::drop_peers_on_cleanup(conn.clone());

    // Audio self-test: whenever a share of ours starts, tap the captured
    // stream for a couple of seconds and warn the sharer if no sound came
    // through (capture failed, muted device, silent source). Cleared as
    // soon as sharing stops.
    #[cfg(feature = "hydrate")]
    {
        let conn_for_probe = conn.clone();
        Effect::new(move |_| {
            if !is_sharing.get() {
                audio_warning.set(None);
                share_has_audio.set(false);
                return;
            }
            let Some(stream) = conn_for_probe.local_stream.borrow().clone() else {
                return;
            };
            // A desktop share that opted out of audio and one whose
            // loopback capture failed both arrive here as a video-only
            // stream, indistinguishable from the renderer. Treat "the
            // stream actually carries an audio track" as the intent: no
            // track means audio simply wasn't part of this share (not a
            // failure to warn about); a silent track is still flagged.
            let has_audio_track = stream_has_audio_track(&stream);
            share_has_audio.set(has_audio_track);
            leptos::task::spawn_local(async move {
                let health =
                    crate::session::audio_health::probe_share_audio(&stream, has_audio_track).await;
                audio_warning.set(health.warning());
            });
        });

        // Applying/clearing the outgoing audio mute. Also resets the toggle
        // when a share ends, so the next share starts un-muted.
        let conn_for_mute = conn.clone();
        Effect::new(move |_| {
            let muted = audio_muted.get();
            if !is_sharing.get() {
                if muted {
                    audio_muted.set(false);
                }
                return;
            }
            crate::session::audio::set_shared_audio_muted(&conn_for_mute, muted);
        });

        // Copy the invite link the moment a share of ours goes live, so
        // there's something ready to paste — the quick-share flow already
        // does this via the desktop shell, so skip it there.
        let quick_share_active = crate::quick_share::requested();
        let room_code_for_copy = initial_code.clone();
        Effect::new(move |_| {
            if is_sharing.get() && !quick_share_active {
                invite::copy_invite_link(&room_code_for_copy, invite_copied);
            }
        });
    }

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
        <div
            class="room-page"
            class:hidden=move || !authenticated.get()
            class:chrome-hidden=move || !controls_visible.get()
        >
            <div class="stage-header">
                <span class=lamp_class></span>
                <span class="status-row__meta">{move || room_name.get().unwrap_or_default()}</span>
                <span class="room-member-count">{move || format!("{}/{}", members.get().len(), MAX_MEMBERS)}</span>
                <span
                    class="share-chip"
                    class:hidden=move || !is_sharing.get()
                    aria-live="polite"
                >
                    <span class="share-chip__dot" aria-hidden="true"></span>
                    "Compartilhando"
                </span>
                <span
                    class="audio-chip"
                    class:audio-chip--muted=audio_muted
                    class:hidden=move || !is_sharing.get()
                    aria-live="polite"
                >
                    <span
                        class="audio-chip__dot"
                        aria-hidden="true"
                        class:hidden=move || !(audio_muted.get() || share_has_audio.get())
                    ></span>
                    <span>
                        {move || {
                            if !sharing_has_audio {
                                // A browser too old for `getDisplayMedia`
                                // (which also can't share video at all).
                                "Áudio indisponível"
                            } else if audio_muted.get() {
                                "Áudio mudo"
                            } else if share_has_audio.get() {
                                "Áudio ligado"
                            } else {
                                // This share carries no audio track — the
                                // sharer didn't include it (no "share tab
                                // audio" tick in the browser, or audio off
                                // in the desktop picker).
                                "Áudio desligado"
                            }
                        }}
                    </span>
                    // The audio self-test's diagnostic, folded into the chip
                    // as a hover-for-detail "!" instead of a loose red
                    // sentence elsewhere in the header. Only meaningful while
                    // a real (unmuted) audio track is being sent.
                    <span
                        class="audio-chip__warn"
                        class:hidden=move || audio_warning.get().is_none() || audio_muted.get()
                        tabindex="0"
                        role="note"
                        aria-label=move || audio_warning.get().unwrap_or_default()
                    >
                        "!"
                        <span class="audio-chip__warn-tip" role="tooltip">
                            {move || audio_warning.get().unwrap_or_default()}
                        </span>
                    </span>
                </span>
                // Surface the status sentence only while something is off or
                // in progress (reconnecting, an error) — the steady "Conectado."
                // state stays silent, represented by the lamp alone.
                <span
                    class="stage-header__status"
                    class:hidden=move || matches!(status_meta(&status.get()).0, "idle" | "live")
                >
                    {move || status.get()}
                </span>
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
                // On a phone this isn't an error — no mobile browser can
                // screen-share — so it reads as a plain note, not red.
                <span
                    class="status-text"
                    class:status-text--error=move || !is_touch.get()
                    class:hidden=move || can_share
                >
                    {move || {
                        if is_touch.get() {
                            "Compartilhar tela não é possível neste aparelho — você pode assistir normalmente."
                        } else {
                            "Seu navegador não suporta compartilhar tela — você ainda pode assistir."
                        }
                    }}
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
                    is_touch,
                    controls_visible,
                })}
            </div>
            <div
                class="room-controls"
                class:room-controls--hidden=move || !controls_visible.get()
                on:mouseenter=move |_| pause_hide_controls()
                on:mouseleave=move |_| resume_hide_controls()
            >
                <div class="control-group">
                    // Touch has no tap-outside-the-video to leave focus with;
                    // this is the way back to the grid. Desktop clicks the
                    // focused card itself, so it only shows on touch.
                    <button
                        class="icon-btn icon-btn--neutral"
                        class:hidden=move || !(is_touch.get() && expanded.get().is_some())
                        title="Voltar para a grade"
                        aria-label="Voltar para a grade"
                        on:click=move |_| expanded.set(None)
                    >
                        {icon_minimize}
                    </button>
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
                        class:hidden=move || !is_sharing.get()
                        title="Trocar a tela ou janela compartilhada"
                        aria-label="Trocar a tela ou janela compartilhada"
                        on:click=switch_source.clone()
                    >
                        {icon_switch}
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
                        on:click=move |_| {
                            let now_hidden = !own_preview_hidden.get_untracked();
                            own_preview_hidden.set(now_hidden);
                            // A hidden preview card leaves the grid; if it was
                            // the expanded one, drop focus so the grid doesn't
                            // stay in focus mode pointing at a card that's gone.
                            if now_hidden && expanded.get_untracked() == my_peer_id.get_untracked() {
                                expanded.set(None);
                            }
                        }
                    >
                        {icon_video_off}
                    </button>
                    <div class="control-group__menu" class:hidden=move || !is_sharing.get()>
                        <TransmissionMenu
                            video_mode=video_mode
                            on_video_mode=set_video_mode
                            audio_preset=audio_preset
                            on_audio_preset=set_audio_preset
                            has_audio=share_has_audio
                            audio_muted=audio_muted
                        />
                    </div>
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
