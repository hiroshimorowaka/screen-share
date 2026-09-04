use leptos::prelude::*;
use leptos_meta::Stylesheet;
use leptos_router::{
    components::{Route, Routes},
    ParamSegment, StaticSegment,
};

use crate::pages::{HomePage, NotFound, RoomPage};
#[cfg(debug_assertions)]
use crate::room::dev_preview::DevRoomPreviewPage;

/// The dev-only room test bench route only exists in debug builds — a
/// release build never compiles `app_routes`'s debug body (the route's own
/// module is likewise `#[cfg(debug_assertions)]`, see `room::dev_preview`),
/// so there's no dev-only path to accidentally ship. `<Routes>` types
/// itself from its exact list of children, which is why this needs two full
/// versions rather than one `<Routes>` with a conditional child inside it.
#[cfg(debug_assertions)]
pub(crate) fn app_routes() -> impl IntoView {
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
pub(crate) fn app_routes() -> impl IntoView {
    view! {
        <Routes fallback=|| view! { <NotFound/> }>
            <Route path=StaticSegment("") view=HomePage/>
            <Route path=(StaticSegment("r"), ParamSegment("code")) view=RoomPage/>
        </Routes>
    }
}
