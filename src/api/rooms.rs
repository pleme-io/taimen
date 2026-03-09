//! Room CRUD, participant management, and recording control endpoints.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::app_state::AppState;
use crate::room::{RoomId, RoomState};

/// Build the room routes.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", post(create_room).get(list_rooms))
        .route("/{room_id}", get(get_room).delete(delete_room))
        .route("/{room_id}/participants", get(list_participants))
        .route("/{room_id}/end", post(end_room))
        .route("/{room_id}/recording/start", post(start_recording))
        .route("/{room_id}/recording/stop", post(stop_recording))
        .route("/{room_id}/recording", get(get_recording_state))
        .route("/{room_id}/state", axum::routing::put(set_room_state))
}

/// Room creation request.
#[derive(Debug, Deserialize)]
struct CreateRoomRequest {
    /// Room name.
    name: String,
    /// Maximum participants (default 100).
    #[serde(default = "default_max_participants")]
    max_participants: usize,
}

fn default_max_participants() -> usize {
    100
}

/// Room response (public view).
#[derive(Debug, Serialize)]
struct RoomResponse {
    id: RoomId,
    name: String,
    host_id: Uuid,
    participant_count: usize,
    max_participants: usize,
    state: RoomState,
    created_at: chrono::DateTime<chrono::Utc>,
}

impl RoomResponse {
    fn from_room(room: &crate::room::Room) -> Self {
        Self {
            id: room.id,
            name: room.name.clone(),
            host_id: room.host_id,
            participant_count: room.participants.len(),
            max_participants: room.max_participants,
            state: room.state,
            created_at: room.created_at,
        }
    }
}

/// `POST /api/v1/rooms`
async fn create_room(
    State(state): State<AppState>,
    Json(body): Json<CreateRoomRequest>,
) -> impl IntoResponse {
    if body.name.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "room name required"})),
        ));
    }

    let host_id = Uuid::new_v4(); // In production, extract from auth token
    let room = state.store.create_room(&body.name, host_id, body.max_participants);

    Ok::<_, (StatusCode, Json<serde_json::Value>)>((
        StatusCode::CREATED,
        Json(serde_json::to_value(RoomResponse::from_room(&room)).unwrap()),
    ))
}

/// `GET /api/v1/rooms`
async fn list_rooms(State(state): State<AppState>) -> impl IntoResponse {
    let rooms: Vec<RoomResponse> = state
        .store
        .list_rooms()
        .iter()
        .map(RoomResponse::from_room)
        .collect();
    Json(rooms)
}

/// `GET /api/v1/rooms/:room_id`
async fn get_room(
    State(state): State<AppState>,
    Path(room_id): Path<RoomId>,
) -> impl IntoResponse {
    let room = state.store.get_room(&room_id).ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "room not found"})),
        )
    })?;
    Ok::<_, (StatusCode, Json<serde_json::Value>)>(Json(
        serde_json::to_value(RoomResponse::from_room(&room)).unwrap(),
    ))
}

/// `DELETE /api/v1/rooms/:room_id`
async fn delete_room(
    State(state): State<AppState>,
    Path(room_id): Path<RoomId>,
) -> impl IntoResponse {
    state.store.delete_room(&room_id).map_err(|e| {
        (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": e.to_string()})),
        )
    })?;
    state.remove_room_channel(&room_id);
    Ok::<_, (StatusCode, Json<serde_json::Value>)>(StatusCode::NO_CONTENT)
}

/// `GET /api/v1/rooms/:room_id/participants`
async fn list_participants(
    State(state): State<AppState>,
    Path(room_id): Path<RoomId>,
) -> impl IntoResponse {
    let participants = state.store.list_participants(&room_id);
    Json(participants)
}

/// `POST /api/v1/rooms/:room_id/end`
async fn end_room(
    State(state): State<AppState>,
    Path(room_id): Path<RoomId>,
) -> impl IntoResponse {
    // Broadcast end room signal
    state.broadcast_to_room(
        room_id,
        crate::signal::SignalMessage::EndRoom { room_id },
    );

    state.store.end_room(&room_id).map_err(|e| {
        (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": e.to_string()})),
        )
    })?;
    state.remove_room_channel(&room_id);

    Ok::<_, (StatusCode, Json<serde_json::Value>)>(StatusCode::NO_CONTENT)
}

/// `POST /api/v1/rooms/:room_id/recording/start`
async fn start_recording(
    State(state): State<AppState>,
    Path(room_id): Path<RoomId>,
) -> impl IntoResponse {
    // In production, extract participant ID from auth token
    let started_by = Uuid::new_v4();
    let info = state.store.start_recording(&room_id, started_by).map_err(|e| {
        (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": e.to_string()})),
        )
    })?;
    Ok::<_, (StatusCode, Json<serde_json::Value>)>(Json(info))
}

/// `POST /api/v1/rooms/:room_id/recording/stop`
async fn stop_recording(
    State(state): State<AppState>,
    Path(room_id): Path<RoomId>,
) -> impl IntoResponse {
    let info = state.store.stop_recording(&room_id).map_err(|e| {
        (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": e.to_string()})),
        )
    })?;
    Ok::<_, (StatusCode, Json<serde_json::Value>)>(Json(info))
}

/// `GET /api/v1/rooms/:room_id/recording`
async fn get_recording_state(
    State(state): State<AppState>,
    Path(room_id): Path<RoomId>,
) -> impl IntoResponse {
    let info = state.store.get_recording_state(&room_id).ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "no recording state"})),
        )
    })?;
    Ok::<_, (StatusCode, Json<serde_json::Value>)>(Json(info))
}

#[derive(Debug, Deserialize)]
struct SetRoomStateRequest {
    state: RoomState,
}

/// `PUT /api/v1/rooms/:room_id/state`
async fn set_room_state(
    State(state): State<AppState>,
    Path(room_id): Path<RoomId>,
    Json(body): Json<SetRoomStateRequest>,
) -> impl IntoResponse {
    let room = state.store.set_room_state(&room_id, body.state).map_err(|e| {
        (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": e.to_string()})),
        )
    })?;
    Ok::<_, (StatusCode, Json<serde_json::Value>)>(Json(
        serde_json::to_value(RoomResponse::from_room(&room)).unwrap(),
    ))
}
