use axum::extract::{Path, State};
use axum::Json;

use super::protocol::RoomStatus;
use super::registry::Registry;

pub async fn room_status_handler(State(registry): State<Registry>, Path(code): Path<String>) -> Json<RoomStatus> {
    match registry.room_status(&code) {
        Some(summary) => Json(RoomStatus {
            exists: true,
            name: Some(summary.name),
            member_count: Some(summary.member_count),
        }),
        None => Json(RoomStatus { exists: false, name: None, member_count: None }),
    }
}
