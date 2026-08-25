use serde::{Deserialize, Serialize};

use crate::ui::profile::{Profile, RecentRoom};

const NICK_KEY: &str = "screen_share_nick";
const PROFILE_KEY: &str = "screen_share_profile";
const RECENT_ROOMS_KEY: &str = "screen_share_recent_rooms";
const LAST_ROOM_NAME_KEY: &str = "screen_share_last_room_name";
const DEVICE_ID_KEY: &str = "screen_share_device_id";
const ROOM_SESSION_KEY_PREFIX: &str = "screen_share_room_session_";
const MAX_RECENT_ROOMS: usize = 10;

#[cfg(feature = "hydrate")]
fn local_storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok()?
}

/// Tab-scoped (unlike everything else in this module, which uses
/// `localStorage`) — cleared the moment the tab/window closes, so a stray
/// reload rejoins silently but actually leaving the browser behind requires
/// the nick/password gate again, same as a fresh visit.
#[cfg(feature = "hydrate")]
fn session_storage() -> Option<web_sys::Storage> {
    web_sys::window()?.session_storage().ok()?
}

#[cfg(feature = "hydrate")]
fn room_session_key(room_code: &str) -> String {
    format!("{ROOM_SESSION_KEY_PREFIX}{room_code}")
}

/// What's needed to silently rejoin a room after a same-tab reload, without
/// showing the nick/password gate again. Unlike `RecentRoom`, this *does*
/// carry the password — `sessionStorage`'s tab-scoped, auto-clearing
/// lifetime is exactly the boundary that makes that acceptable here.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RoomSession {
    pub nick: String,
    pub color: String,
    pub password: Option<String>,
}

#[cfg(not(feature = "hydrate"))]
pub fn load_room_session(_room_code: &str) -> Option<RoomSession> {
    None
}

#[cfg(feature = "hydrate")]
pub fn load_room_session(room_code: &str) -> Option<RoomSession> {
    let json = session_storage()?.get_item(&room_session_key(room_code)).ok()??;
    serde_json::from_str(&json).ok()
}

#[cfg(not(feature = "hydrate"))]
pub fn save_room_session(_room_code: &str, _session: &RoomSession) {}

#[cfg(feature = "hydrate")]
pub fn save_room_session(room_code: &str, session: &RoomSession) {
    if let (Some(storage), Ok(json)) = (session_storage(), serde_json::to_string(session)) {
        let _ = storage.set_item(&room_session_key(room_code), &json);
    }
}

#[cfg(not(feature = "hydrate"))]
pub fn clear_room_session(_room_code: &str) {}

#[cfg(feature = "hydrate")]
pub fn clear_room_session(room_code: &str) {
    if let Some(storage) = session_storage() {
        let _ = storage.remove_item(&room_session_key(room_code));
    }
}

#[cfg(not(feature = "hydrate"))]
pub fn load_profile() -> Profile {
    Profile::default()
}

#[cfg(feature = "hydrate")]
pub fn load_profile() -> Profile {
    let Some(json) = local_storage().and_then(|s| s.get_item(PROFILE_KEY).ok()?) else {
        return Profile::default();
    };
    serde_json::from_str(&json).unwrap_or_default()
}

#[cfg(not(feature = "hydrate"))]
pub fn save_profile(_profile: &Profile) {}

#[cfg(feature = "hydrate")]
pub fn save_profile(profile: &Profile) {
    if let (Some(storage), Ok(json)) = (local_storage(), serde_json::to_string(profile)) {
        let _ = storage.set_item(PROFILE_KEY, &json);
    }
}

#[cfg(not(feature = "hydrate"))]
pub fn load_recent_rooms() -> Vec<RecentRoom> {
    Vec::new()
}

#[cfg(feature = "hydrate")]
pub fn load_recent_rooms() -> Vec<RecentRoom> {
    let Some(json) = local_storage().and_then(|s| s.get_item(RECENT_ROOMS_KEY).ok()?) else {
        return Vec::new();
    };
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
    if let (Some(storage), Ok(json)) = (local_storage(), serde_json::to_string(rooms)) {
        let _ = storage.set_item(RECENT_ROOMS_KEY, &json);
    }
}

#[cfg(not(feature = "hydrate"))]
pub fn load_nick() -> Option<String> {
    None
}

#[cfg(feature = "hydrate")]
pub fn load_nick() -> Option<String> {
    local_storage()?.get_item(NICK_KEY).ok()?
}

#[cfg(not(feature = "hydrate"))]
pub fn save_nick(_nick: &str) {}

#[cfg(feature = "hydrate")]
pub fn save_nick(nick: &str) {
    if let Some(storage) = local_storage() {
        let _ = storage.set_item(NICK_KEY, nick);
    }
}

#[cfg(not(feature = "hydrate"))]
pub fn load_last_room_name() -> Option<String> {
    None
}

#[cfg(feature = "hydrate")]
pub fn load_last_room_name() -> Option<String> {
    local_storage()?.get_item(LAST_ROOM_NAME_KEY).ok()?
}

#[cfg(not(feature = "hydrate"))]
pub fn save_last_room_name(_room_name: &str) {}

#[cfg(feature = "hydrate")]
pub fn save_last_room_name(room_name: &str) {
    if let Some(storage) = local_storage() {
        let _ = storage.set_item(LAST_ROOM_NAME_KEY, room_name);
    }
}

/// Unlike nick/color, `device_id` never appears on screen — it can be read
/// directly, without the async post-mount load pattern, with no risk of a
/// hydration mismatch.
#[cfg(not(feature = "hydrate"))]
pub fn ensure_device_id() -> String {
    String::new()
}

#[cfg(feature = "hydrate")]
pub fn ensure_device_id() -> String {
    let Some(storage) = local_storage() else { return String::new() };

    if let Ok(Some(existing)) = storage.get_item(DEVICE_ID_KEY) {
        return existing;
    }

    let Some(window) = web_sys::window() else { return String::new() };
    let Ok(crypto) = window.crypto() else { return String::new() };
    let id = crypto.random_uuid();
    let _ = storage.set_item(DEVICE_ID_KEY, &id);
    id
}
