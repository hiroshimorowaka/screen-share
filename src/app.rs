use leptos::prelude::*;
use leptos_meta::{provide_meta_context, MetaTags, Stylesheet, Title};
use leptos_router::{
    components::{Route, Router, Routes},
    StaticSegment,
};

pub fn shell(options: LeptosOptions) -> impl IntoView {
    view! {
        <!DOCTYPE html>
        <html lang="pt-BR">
            <head>
                <meta charset="utf-8"/>
                <meta name="viewport" content="width=device-width, initial-scale=1"/>
                <AutoReload options=options.clone() />
                <HydrationScripts options/>
                <MetaTags/>
            </head>
            <body>
                <App/>
            </body>
        </html>
    }
}

#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();

    view! {
        <Stylesheet id="leptos" href="/pkg/screen_share.css"/>
        <Title text="Compartilhamento de tela"/>
        <Router>
            <main>
                <Routes fallback=|| "Página não encontrada.".into_view()>
                    <Route path=StaticSegment("") view=HomePlaceholder/>
                </Routes>
            </main>
        </Router>
    }
}

#[component]
fn HomePlaceholder() -> impl IntoView {
    view! { <h1>"Compartilhamento de tela"</h1> }
}
