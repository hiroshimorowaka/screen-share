use leptos::prelude::*;

/// The router's fallback view for an unknown URL — reuses the lobby's
/// shell (wordmark + dark ground) so a mistyped link still lands
/// somewhere that looks like the product.
#[component]
pub fn NotFound() -> impl IntoView {
    view! {
        <div class="lobby not-found">
            <header class="lobby__bar">
                <span class="wordmark">"screenshare"<span class="wordmark__dot"></span></span>
            </header>
            <section class="not-found__body">
                <p class="not-found__code">"Erro 404"</p>
                <h1 class="not-found__title">"Esta página não existe."</h1>
                <p class="not-found__lead">
                    "O link pode estar quebrado, verifique e tente novamente."
                </p>
                <a class="btn btn--primary not-found__cta" href="/">"Voltar ao início"</a>
            </section>
        </div>
    }
}
