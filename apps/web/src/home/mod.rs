//! The lobby feature slice: the `/` route component (`page`), plus
//! create-a-room, join-by-code / link, and the "recent rooms" list.

pub(crate) mod components;
pub(crate) mod create;
pub(crate) mod join;
mod page;
pub(crate) mod recent;
pub(crate) mod state;

pub use page::HomePage;
pub(crate) use state::HomeState;
