//! The isomorphic app root: the HTML shell rendered server-side and
//! the `<App>` component that hydrates it. The route table is in
//! `router`.

mod router;

use leptos::prelude::*;
use leptos_meta::{provide_meta_context, MetaTags, Stylesheet, Title};
use leptos_router::components::Router;

use router::app_routes;

pub fn shell(options: LeptosOptions) -> impl IntoView {
    view! {
        <!DOCTYPE html>
        <html lang="pt-BR">
            <head>
                <meta charset="utf-8"/>
                // `viewport-fit=cover` lets the page paint edge to edge and
                // exposes `env(safe-area-inset-*)` so the fixed room chrome
                // can clear the notch / home indicator.
                <meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover"/>
                // Space Grotesk (display) + Space Mono (data readouts),
                // pulled from Google Fonts rather than vendored.
                <link rel="preconnect" href="https://fonts.googleapis.com"/>
                <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin="anonymous"/>
                <link
                    rel="stylesheet"
                    href="https://fonts.googleapis.com/css2?family=Space+Grotesk:wght@500;700&family=Space+Mono:wght@400;700&display=swap"
                />
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
        <Stylesheet id="tokens" href="/styles/tokens.css"/>
        <Stylesheet id="base" href="/styles/base.css"/>
        <Stylesheet id="home" href="/styles/home.css"/>
        <Stylesheet id="room" href="/styles/room.css"/>
        <Stylesheet id="room-transmission-menu" href="/styles/room-transmission-menu.css"/>
        <Stylesheet id="card" href="/styles/card.css"/>
        <Stylesheet id="card-widgets" href="/styles/card-widgets.css"/>
        <Title text="Compartilhamento de tela"/>
        <Router>
            <main>
                {app_routes()}
            </main>
        </Router>
    }
}
