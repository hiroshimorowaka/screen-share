use leptos::prelude::*;
use leptos_meta::{provide_meta_context, MetaTags, Stylesheet, Title};
use leptos_router::{
    components::{Route, Router, Routes},
    ParamSegment, StaticSegment,
};

use crate::features::home::HomePage;
use crate::features::not_found::NotFound;
#[cfg(debug_assertions)]
use crate::features::room::DevRoomPreviewPage;
use crate::features::room::RoomPage;

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
                // pulled from Google Fonts rather than vendored — see
                // docs/decisions/0006-visual-redesign.md.
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
        <Stylesheet id="card" href="/styles/card.css"/>
        <Title text="Compartilhamento de tela"/>
        <Router>
            <main>
                {app_routes()}
            </main>
        </Router>
    }
}

/// The dev-only room test bench route only exists in debug builds — a
/// release build never compiles `app_routes_debug`'s body (the route's own
/// module is likewise `#[cfg(debug_assertions)]`, see `room/mod.rs`), so
/// there's no dev-only path to accidentally ship. `<Routes>` types itself
/// from its exact list of children, which is why this needs two full
/// versions rather than one `<Routes>` with a conditional child inside it.
#[cfg(debug_assertions)]
fn app_routes() -> impl IntoView {
    view! {
        <Stylesheet id="dev-preview" href="/styles/dev_preview.css"/>
        <Routes fallback=|| view! { <NotFound/> }>
            <Route path=StaticSegment("") view=HomePage/>
            <Route path=(StaticSegment("r"), ParamSegment("code")) view=RoomPage/>
            <Route path=(StaticSegment("dev"), StaticSegment("room-preview")) view=DevRoomPreviewPage/>
        </Routes>
    }
}

#[cfg(not(debug_assertions))]
fn app_routes() -> impl IntoView {
    view! {
        <Routes fallback=|| view! { <NotFound/> }>
            <Route path=StaticSegment("") view=HomePage/>
            <Route path=(StaticSegment("r"), ParamSegment("code")) view=RoomPage/>
        </Routes>
    }
}
