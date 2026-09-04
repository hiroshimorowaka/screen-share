//! The pre-authentication panels shown before a member is in the room:
//! "checking…", "not found", and the nick / colour / password join form.
//! Exactly one is visible at a time (none once `authenticated`), driven by
//! `room_exists`.

use leptos::prelude::*;

use crate::components::color_picker::ColorPicker;
use crate::components::status_message::StatusMessage;

#[component]
pub(super) fn RoomGate<J>(
    /// The room code from the route, shown above each panel.
    #[prop(into)]
    code: Signal<String>,
    authenticated: ReadSignal<bool>,
    /// `None` while the room check is in flight, then `Some(exists)`.
    room_exists: ReadSignal<Option<bool>>,
    requires_password: ReadSignal<bool>,
    nick: ReadSignal<String>,
    set_nick: WriteSignal<String>,
    color: ReadSignal<String>,
    set_color: WriteSignal<String>,
    password: ReadSignal<String>,
    set_password: WriteSignal<String>,
    status: ReadSignal<String>,
    set_status: WriteSignal<String>,
    /// Route code, for persisting a successful join to `localStorage`.
    room_code: String,
    /// Opens the WebSocket and joins with `(nick, color, password)`.
    on_join: J,
) -> impl IntoView
where
    J: Fn(String, String, Option<String>) + Clone + 'static,
{
    let manual_join = {
        // Only read from the `#[cfg(feature = "hydrate")]` block below — an
        // `ssr`-only compile sees no reads and would otherwise flag it.
        #[cfg_attr(not(feature = "hydrate"), allow(unused_variables))]
        let join_room_code = room_code;
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
            crate::client::storage::save_room_session(
                &join_room_code,
                &crate::client::storage::RoomSession {
                    nick: nick_value.clone(),
                    color: color.get_untracked(),
                    password: password_value.clone(),
                },
            );
            on_join(nick_value, color.get_untracked(), password_value);
        }
    };

    view! {
        <div
            class="panel"
            class:hidden=move || authenticated.get() || room_exists.get().is_some()
        >
            <h1>"Verificando sala..."</h1>
            <p class="status-row__meta">{move || code.get()}</p>
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
        <div
            class="panel"
            class:hidden=move || authenticated.get() || room_exists.get() != Some(true)
        >
            <h1>"Entrar na sala"</h1>
            // Just the code here: the room name isn't known until the
            // `Joined` snapshot (finding F06), and this panel is pre-join.
            <p class="status-row__meta">{move || code.get()}</p>
            <form on:submit=manual_join>
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
                <label class="field" class:hidden=move || !requires_password.get()>
                    <span class="field__label">"Senha da sala"</span>
                    <input
                        class="field__input"
                        type="password"
                        prop:value=password
                        on:input:target=move |ev| set_password.set(ev.target().value())
                    />
                </label>
                <button class="btn btn--primary" type="submit">
                    "Entrar"
                </button>
            </form>
            <StatusMessage status=status/>
        </div>
    }
}
