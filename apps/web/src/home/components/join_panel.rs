//! The "Entrar em uma sala" panel: paste a code or invite link, plus the
//! "salas recentes" list this browser remembers. Reads `HomeState` from
//! context; the submit handler is passed in by `pages::home`.

use leptos::prelude::*;

use crate::home::HomeState;

#[component]
pub(crate) fn JoinRoomPanel<F>(on_submit: F) -> impl IntoView
where
    F: Fn(leptos::ev::SubmitEvent) + 'static,
{
    let HomeState {
        join_input,
        set_join_input,
        join_status,
        recent_rooms,
        ..
    } = expect_context::<HomeState>();

    view! {
        <div class="panel">
            <h2 class="panel__title">"Entrar em uma sala"</h2>
            <p class="subtext">"Cole o código da sala ou o link completo do convite — você poderá informar o nick e a senha na página da sala."</p>

            <form on:submit=on_submit>
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
                <p class="recent-rooms__label">"Salas recentes"</p>
                <For each=move || recent_rooms.get() key=|r| r.code.clone() let(room)>
                    <a class="recent-room" href=format!("/r/{}", room.code)>
                        <span class="recent-room__name">{room.name.clone()}</span>
                        <div class="recent-room__meta">
                            <span class="recent-room__code">{room.code.clone()}</span>
                        </div>
                    </a>
                </For>
            </div>
        </div>
    }
}
