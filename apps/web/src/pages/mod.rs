//! One thin component per route. Each composes a feature slice
//! (`home`, `room`) or renders standalone chrome (`not_found`); the
//! `<Routes>` table lives in `app::router`.

mod home;
mod not_found;
mod room;

pub use home::HomePage;
pub use not_found::NotFound;
pub use room::RoomPage;
