use leptos::prelude::*;
use leptos_router::hooks::use_params_map;

#[component]
pub fn RoomPage() -> impl IntoView {
    let params = use_params_map();
    let code = move || params.read().get("code").unwrap_or_default();

    view! { <h1>"Assistindo sala " {code}</h1> }
}
