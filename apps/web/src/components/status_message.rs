use leptos::prelude::*;

use screen_share_domain::status::status_meta;

#[component]
pub fn StatusMessage(status: ReadSignal<String>) -> impl IntoView {
    view! {
        <p class="status-text" class:status-text--error=move || status_meta(&status.get()).0 == "error">
            {status}
        </p>
    }
}
