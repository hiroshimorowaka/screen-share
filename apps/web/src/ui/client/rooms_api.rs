use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::Response;

use crate::signaling::protocol::RoomStatus;

/// `None` means a network/parsing failure (inconclusive) — only an actual
/// `RoomStatus { exists: false, .. }` means "the room doesn't exist".
pub async fn check_room(code: &str) -> Option<RoomStatus> {
    let window = web_sys::window()?;
    let promise = window.fetch_with_str(&format!("/api/rooms/{code}"));
    let resp_value = JsFuture::from(promise).await.ok()?;
    let response: Response = resp_value.dyn_into().ok()?;
    let text_promise = response.text().ok()?;
    let text_value = JsFuture::from(text_promise).await.ok()?;
    let text = text_value.as_string()?;
    serde_json::from_str(&text).ok()
}
