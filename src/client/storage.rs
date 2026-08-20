use crate::profile::{Profile, RecentRoom};

const NICK_KEY: &str = "screen_share_nick";
const PROFILE_KEY: &str = "screen_share_profile";
const RECENT_ROOMS_KEY: &str = "screen_share_recent_rooms";
const MAX_RECENT_ROOMS: usize = 10;

#[cfg(not(feature = "hydrate"))]
pub fn load_profile() -> Profile {
    Profile::default()
}

#[cfg(feature = "hydrate")]
pub fn load_profile() -> Profile {
    let Some(window) = web_sys::window() else { return Profile::default() };
    let Ok(Some(storage)) = window.local_storage() else { return Profile::default() };
    let Ok(Some(json)) = storage.get_item(PROFILE_KEY) else { return Profile::default() };
    serde_json::from_str(&json).unwrap_or_default()
}

#[cfg(not(feature = "hydrate"))]
pub fn save_profile(_profile: &Profile) {}

#[cfg(feature = "hydrate")]
pub fn save_profile(profile: &Profile) {
    if let Some(window) = web_sys::window() {
        if let Ok(Some(storage)) = window.local_storage() {
            if let Ok(json) = serde_json::to_string(profile) {
                let _ = storage.set_item(PROFILE_KEY, &json);
            }
        }
    }
}

#[cfg(not(feature = "hydrate"))]
pub fn load_recent_rooms() -> Vec<RecentRoom> {
    Vec::new()
}

#[cfg(feature = "hydrate")]
pub fn load_recent_rooms() -> Vec<RecentRoom> {
    let Some(window) = web_sys::window() else { return Vec::new() };
    let Ok(Some(storage)) = window.local_storage() else { return Vec::new() };
    let Ok(Some(json)) = storage.get_item(RECENT_ROOMS_KEY) else { return Vec::new() };
    serde_json::from_str(&json).unwrap_or_default()
}

#[cfg(not(feature = "hydrate"))]
pub fn save_recent_room(_room: RecentRoom) {}

#[cfg(feature = "hydrate")]
pub fn save_recent_room(room: RecentRoom) {
    let mut rooms = load_recent_rooms();
    rooms.retain(|r| r.code != room.code);
    rooms.insert(0, room);
    rooms.truncate(MAX_RECENT_ROOMS);
    save_recent_rooms_list(&rooms);
}

#[cfg(not(feature = "hydrate"))]
pub fn remove_recent_room(_code: &str) {}

#[cfg(feature = "hydrate")]
pub fn remove_recent_room(code: &str) {
    let mut rooms = load_recent_rooms();
    rooms.retain(|r| r.code != code);
    save_recent_rooms_list(&rooms);
}

#[cfg(feature = "hydrate")]
fn save_recent_rooms_list(rooms: &[RecentRoom]) {
    if let Some(window) = web_sys::window() {
        if let Ok(Some(storage)) = window.local_storage() {
            if let Ok(json) = serde_json::to_string(rooms) {
                let _ = storage.set_item(RECENT_ROOMS_KEY, &json);
            }
        }
    }
}

#[cfg(not(feature = "hydrate"))]
pub fn load_nick() -> Option<String> {
    None
}

#[cfg(feature = "hydrate")]
pub fn load_nick() -> Option<String> {
    let window = web_sys::window()?;
    let storage = window.local_storage().ok()??;
    storage.get_item(NICK_KEY).ok()?
}

#[cfg(not(feature = "hydrate"))]
pub fn save_nick(_nick: &str) {}

#[cfg(feature = "hydrate")]
pub fn save_nick(nick: &str) {
    if let Some(window) = web_sys::window() {
        if let Ok(Some(storage)) = window.local_storage() {
            let _ = storage.set_item(NICK_KEY, nick);
        }
    }
}
