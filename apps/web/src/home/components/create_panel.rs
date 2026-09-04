//! The "Criar sala" panel: nick, colour, room name, and the mutually
//! exclusive "sala pública" toggle / password field. Reads `HomeState`
//! from context; the submit handler is passed in by `pages::home`.

use leptos::prelude::*;

use crate::components::ui::color_picker::ColorPicker;
use crate::components::ui::status_message::StatusMessage;
use crate::home::HomeState;

#[component]
pub(crate) fn CreateRoomPanel<F>(on_submit: F) -> impl IntoView
where
    F: Fn(leptos::ev::SubmitEvent) + 'static,
{
    let HomeState {
        nick,
        set_nick,
        color,
        set_color,
        room_name,
        set_room_name,
        password,
        set_password,
        public_room,
        set_public_room,
        status,
        submitting,
        ..
    } = expect_context::<HomeState>();

    view! {
        <div class="panel">
            <h2 class="panel__title">"Criar sala"</h2>
            <p class="subtext">"Escolha um nick, uma cor e um nome."</p>

            <form on:submit=on_submit>
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
                <div class="switch-field">
                    <label class="switch">
                        <input
                            type="checkbox"
                            class="switch__input"
                            prop:checked=public_room
                            on:change:target=move |ev| set_public_room.set(ev.target().checked())
                        />
                        <span class="switch__track"><span class="switch__thumb"></span></span>
                        <span class="switch__label">"Sala pública"</span>
                    </label>
                    <p class="switch-field__hint">
                        {move || {
                            if public_room.get() {
                                "Qualquer pessoa com o link entra, sem senha."
                            } else {
                                "Só entra quem tiver o link e a senha abaixo."
                            }
                        }}
                    </p>
                </div>
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
    }
}
