use leptos::prelude::*;

use screen_share_domain::status::{status_kind, StatusKind};

#[component]
pub fn StatusMessage(status: ReadSignal<String>) -> impl IntoView {
    view! {
        <p class="status-text" class:status-text--error=move || status_kind(&status.get()) == StatusKind::Error>
            {status}
        </p>
    }
}
