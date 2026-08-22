use leptos::prelude::*;
use leptos_meta::{provide_meta_context, MetaTags, Stylesheet, Title};
use leptos_router::{
    components::{Route, Router, Routes},
    ParamSegment, StaticSegment,
};

use crate::ui::pages::home::HomePage;
use crate::ui::pages::room::RoomPage;
#[cfg(debug_assertions)]
use crate::ui::pages::room::DevRoomPreviewPage;

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
        <Routes fallback=|| "Página não encontrada.".into_view()>
            <Route path=StaticSegment("") view=HomePage/>
            <Route path=(StaticSegment("r"), ParamSegment("code")) view=RoomPage/>
            <Route path=(StaticSegment("dev"), StaticSegment("room-preview")) view=DevRoomPreviewPage/>
        </Routes>
    }
}

#[cfg(not(debug_assertions))]
fn app_routes() -> impl IntoView {
    view! {
        <Routes fallback=|| "Página não encontrada.".into_view()>
            <Route path=StaticSegment("") view=HomePage/>
            <Route path=(StaticSegment("r"), ParamSegment("code")) view=RoomPage/>
        </Routes>
    }
}
