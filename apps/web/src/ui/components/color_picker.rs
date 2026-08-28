use leptos::prelude::*;

use crate::ui::components::palette::{color_hex, palette_ids};

#[component]
pub fn ColorPicker(selected: ReadSignal<String>, on_select: WriteSignal<String>) -> impl IntoView {
    view! {
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
                                class:color-swatch--selected=move || selected.get() == id
                                style=format!("background-color: {border}")
                                on:click=move |_| on_select.set(id.to_string())
                            ></button>
                        }
                    })
                    .collect::<Vec<_>>()}
            </div>
        </div>
    }
}
