//! Browser (`wasm32`) tests for `storage` — they exercise the real
//! `localStorage` / `sessionStorage` round trips the `ssr` unit tests
//! can't reach. Run with:
//!
//! ```text
//! cargo test -p screen_share --target wasm32-unknown-unknown \
//!   --no-default-features --features hydrate
//! ```

use wasm_bindgen_test::*;

use super::*;
use crate::features::profile::{Profile, RecentRoom};

wasm_bindgen_test_configure!(run_in_browser);

/// Every test shares one browser page, so wipe both stores first.
fn reset_storage() {
    if let Some(storage) = local_storage() {
        let _ = storage.clear();
    }
    if let Some(storage) = session_storage() {
        let _ = storage.clear();
    }
}

fn room(code: &str, name: &str) -> RecentRoom {
    RecentRoom {
        code: code.to_string(),
        name: name.to_string(),
    }
}

#[wasm_bindgen_test]
fn recent_rooms_round_trip_newest_first() {
    reset_storage();
    save_recent_room(room("AAAA1111", "Sala A"));
    save_recent_room(room("BBBB2222", "Sala B"));

    let rooms = load_recent_rooms();
    assert_eq!(rooms.len(), 2);
    assert_eq!(rooms[0].code, "BBBB2222");
    assert_eq!(rooms[1].code, "AAAA1111");
}

#[wasm_bindgen_test]
fn recent_rooms_dedup_moves_an_existing_code_to_the_front() {
    reset_storage();
    save_recent_room(room("AAAA1111", "Sala A"));
    save_recent_room(room("BBBB2222", "Sala B"));
    save_recent_room(room("AAAA1111", "Sala A renomeada"));

    let rooms = load_recent_rooms();
    assert_eq!(rooms.len(), 2, "the repeated code must not add a row");
    assert_eq!(rooms[0].code, "AAAA1111");
    assert_eq!(rooms[0].name, "Sala A renomeada");
}

#[wasm_bindgen_test]
fn recent_rooms_are_truncated_to_the_cap_keeping_the_newest() {
    reset_storage();
    for i in 0..(MAX_RECENT_ROOMS + 3) {
        save_recent_room(room(&format!("CODE{i:04}"), &format!("Sala {i}")));
    }

    let rooms = load_recent_rooms();
    assert_eq!(rooms.len(), MAX_RECENT_ROOMS);
    assert_eq!(rooms[0].code, format!("CODE{:04}", MAX_RECENT_ROOMS + 2));
    assert!(rooms.iter().all(|r| r.code != "CODE0000"), "oldest dropped");
}

#[wasm_bindgen_test]
fn remove_recent_room_drops_only_the_named_code() {
    reset_storage();
    save_recent_room(room("AAAA1111", "A"));
    save_recent_room(room("BBBB2222", "B"));
    remove_recent_room("AAAA1111");

    let rooms = load_recent_rooms();
    assert_eq!(rooms.len(), 1);
    assert_eq!(rooms[0].code, "BBBB2222");
}

#[wasm_bindgen_test]
fn load_recent_rooms_is_empty_when_the_stored_value_is_corrupt() {
    reset_storage();
    local_storage()
        .unwrap()
        .set_item(RECENT_ROOMS_KEY, "{not json")
        .unwrap();

    assert!(load_recent_rooms().is_empty());
}

#[wasm_bindgen_test]
fn room_session_round_trips_through_session_storage_and_clears() {
    reset_storage();
    let session = RoomSession {
        nick: "Ana".to_string(),
        color: "coral".to_string(),
        password: Some("senha123".to_string()),
    };

    save_room_session("ROOM1", &session);
    assert_eq!(load_room_session("ROOM1"), Some(session));

    clear_room_session("ROOM1");
    assert_eq!(load_room_session("ROOM1"), None);
}

#[wasm_bindgen_test]
fn profile_round_trips_and_falls_back_to_default_on_corrupt_json() {
    reset_storage();
    let profile = Profile {
        nick: "Bia".to_string(),
        color: "sky".to_string(),
    };
    save_profile(&profile);
    assert_eq!(load_profile(), profile);

    local_storage()
        .unwrap()
        .set_item(PROFILE_KEY, "nope")
        .unwrap();
    assert_eq!(load_profile(), Profile::default());
}

#[wasm_bindgen_test]
fn ensure_device_id_generates_once_then_returns_the_same_value() {
    reset_storage();
    let first = ensure_device_id();
    assert!(!first.is_empty());
    assert_eq!(ensure_device_id(), first);
}
