use axum::extract::{Path, State};
use axum::Json;

use super::registry::Registry;
use screen_share_protocol::RoomStatus;

pub async fn room_status_handler(
    State(registry): State<Registry>,
    Path(code): Path<String>,
) -> Json<RoomStatus> {
    match registry.room_status(&code) {
        Some(summary) => Json(RoomStatus {
            exists: true,
            name: Some(summary.name),
            member_count: Some(summary.member_count),
            requires_password: Some(summary.requires_password),
        }),
        None => Json(RoomStatus {
            exists: false,
            name: None,
            member_count: None,
            requires_password: None,
        }),
    }
}
