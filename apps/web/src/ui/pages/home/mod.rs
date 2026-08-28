mod create_room;
mod join_room;
mod recent_rooms;

use std::collections::HashMap;

use leptos::prelude::*;

use create_room::{
    create_room_handler, load_last_room_name_after_mount, start_quick_share_after_mount,
};
use join_room::join_room_handler;
use recent_rooms::{load_recent_rooms_after_mount, prune_recent_rooms};

use crate::signaling::protocol::MAX_MEMBERS;
use crate::ui::components::color_picker::ColorPicker;
use crate::ui::components::status_message::StatusMessage;

#[component]
pub fn HomePage() -> impl IntoView {
    // Signals start at the value SSR would use (empty/default); the real
    // localStorage value is only applied after mount, or Leptos hydration
    // breaks (class bindings react to the wrong value, and the `recent_rooms`
    // `<For>` diverges in length from what the server rendered).
    let (nick, set_nick) = signal(String::new());
    let (color, set_color) = signal(crate::ui::components::palette::DEFAULT_COLOR.to_string());
    crate::ui::profile::load_profile_after_mount(set_nick, set_color);
    let (room_name, set_room_name) = signal(String::new());
    load_last_room_name_after_mount(set_room_name);
    let (password, set_password) = signal(String::new());
    let (public_room, set_public_room) = signal(false);
    let (status, set_status) = signal("Pronto para criar uma sala.".to_string());
    let (submitting, set_submitting) = signal(false);
    let (recent_rooms, set_recent_rooms) = signal(Vec::<crate::ui::profile::RecentRoom>::new());
    // Member count per room: unlike `recent_rooms`, this always comes from
    // the server — it changes on every join/leave and is never persisted in
    // the browser.
    let (member_counts, set_member_counts) = signal(HashMap::<String, usize>::new());

    load_recent_rooms_after_mount(set_recent_rooms);
    prune_recent_rooms(set_recent_rooms, set_member_counts);

    let create_room = create_room_handler(
        nick,
        color,
        room_name,
        password,
        public_room,
        set_status,
        set_submitting,
    );
    start_quick_share_after_mount(set_status, set_submitting);

    let (join_input, set_join_input) = signal(String::new());
    let (join_status, set_join_status) = signal(String::new());
    let join_room = join_room_handler(join_input, set_join_status);

    view! {
        <div class="home-layout">
        <div class="panel">
            <h1>"Criar sala"</h1>
            <p class="subtext">"Escolha um nick, uma cor e um nome. Defina uma senha ou marque a sala como pública — qualquer pessoa com o link entra numa sala pública."</p>

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
                <ColorPicker selected=color on_select=set_color/>
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
                <label class="checkbox-field">
                    <input
                        type="checkbox"
                        prop:checked=public_room
                        on:change:target=move |ev| set_public_room.set(ev.target().checked())
                    />
                    "Sala pública (sem senha)"
                </label>
                <label class="field" class:hidden=move || public_room.get()>
                    <span class="field__label">"Senha da sala"</span>
                    <input
                        class="field__input"
                        type="password"
                        prop:value=password
                        disabled=move || public_room.get()
                        on:input:target=move |ev| set_password.set(ev.target().value())
                    />
                </label>
                <button class="btn btn--primary" type="submit" disabled=submitting>
                    {move || if submitting.get() { "Criando..." } else { "Criar sala" }}
                </button>
            </form>

            <StatusMessage status=status/>
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
        </div>
    }
}
