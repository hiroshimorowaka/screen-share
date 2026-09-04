//! The lobby feature slice: create a room, join by code / link, and the
//! "recent rooms" list. The route component is `pages::home`.

pub(crate) mod components;
pub(crate) mod create;
pub(crate) mod join;
pub(crate) mod recent;
pub(crate) mod state;

pub(crate) use state::HomeState;
