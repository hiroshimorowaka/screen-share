const NICK_KEY: &str = "screen_share_nick";

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
