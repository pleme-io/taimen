//! WebSocket signaling handler.
//!
//! Manages per-room WebSocket connections, relays `SignalMessage` between
//! participants, and handles room lifecycle events.

use axum::extract::ws::{Message, WebSocket};
use axum::extract::{Path, Query, State, WebSocketUpgrade};
use axum::response::IntoResponse;
use futures::{SinkExt, StreamExt};
use uuid::Uuid;

use crate::app_state::AppState;
use crate::participant::{ParticipantId, ParticipantRole};
use crate::room::RoomId;
use crate::signal::{MuteKind, SignalMessage};

/// Query parameters for the WebSocket endpoint.
#[derive(Debug, serde::Deserialize)]
pub struct WsQuery {
    /// Display name for the participant.
    pub name: Option<String>,
    /// Optional auth token.
    pub token: Option<String>,
}

/// WebSocket upgrade handler at `/ws/{room_id}`.
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Path(room_id): Path<RoomId>,
    Query(query): Query<WsQuery>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, room_id, query, state))
}

async fn handle_socket(socket: WebSocket, room_id: RoomId, query: WsQuery, state: AppState) {
    let (mut sender, mut receiver) = socket.split();

    // Wait for the Join message from the client
    let first_msg = match receiver.next().await {
        Some(Ok(Message::Text(text))) => {
            serde_json::from_str::<SignalMessage>(&text).ok()
        }
        _ => None,
    };

    let (join_room_id, participant_id, display_name) = match first_msg {
        Some(SignalMessage::Join {
            room_id: rid,
            participant_id: pid,
            display_name,
        }) => (rid, pid, display_name),
        _ => {
            // If no valid Join message, try using query params
            let display_name = query.name.unwrap_or_else(|| "Anonymous".into());
            let pid = Uuid::new_v4();
            (room_id, pid, display_name)
        }
    };

    // Create or get the room
    let room = state.store.get_room(&join_room_id).unwrap_or_else(|| {
        state.store.create_room("Meeting", participant_id, 100)
    });
    let actual_room_id = room.id;

    // Join the room
    let user_id = Uuid::new_v4();
    let participant = match state.store.join_room(
        &actual_room_id,
        user_id,
        &display_name,
        ParticipantRole::Participant,
    ) {
        Ok(p) => p,
        Err(e) => {
            let err_msg = serde_json::json!({"error": e.to_string()});
            let _ = sender.send(Message::Text(err_msg.to_string().into())).await;
            return;
        }
    };

    let pid = participant.id;

    // Broadcast join to existing participants
    state.broadcast_to_room(
        actual_room_id,
        SignalMessage::Join {
            room_id: actual_room_id,
            participant_id: pid,
            display_name: display_name.clone(),
        },
    );

    // Subscribe to room events
    let mut room_rx = state.room_subscribe(actual_room_id);

    // Spawn task to forward room events to this WebSocket
    let send_task = tokio::spawn(async move {
        while let Ok(msg) = room_rx.recv().await {
            let json = match serde_json::to_string(&msg) {
                Ok(j) => j,
                Err(_) => continue,
            };
            if sender.send(Message::Text(json.into())).await.is_err() {
                break;
            }
        }
    });

    // Process incoming messages from this client
    let state_clone = state.clone();
    let recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            match msg {
                Message::Text(text) => {
                    if let Ok(signal) = serde_json::from_str::<SignalMessage>(&text) {
                        handle_signal(&state_clone, actual_room_id, pid, signal);
                    }
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
    });

    // Wait for either task to finish
    tokio::select! {
        _ = send_task => {},
        _ = recv_task => {},
    }

    // Clean up: remove participant and broadcast leave
    let _ = state.store.leave_room(&actual_room_id, &pid);
    state.broadcast_to_room(
        actual_room_id,
        SignalMessage::Leave {
            room_id: actual_room_id,
            participant_id: pid,
        },
    );

    // If room is empty, clean up the broadcast channel
    if state.store.list_participants(&actual_room_id).is_empty() {
        state.remove_room_channel(&actual_room_id);
    }
}

fn handle_signal(state: &AppState, room_id: RoomId, sender_pid: ParticipantId, msg: SignalMessage) {
    match msg {
        // Relay SDP and ICE directly to the target participant
        SignalMessage::Offer { .. }
        | SignalMessage::Answer { .. }
        | SignalMessage::IceCandidate { .. } => {
            // These are peer-to-peer messages routed via the server
            state.broadcast_to_room(room_id, msg);
        }

        // Control messages: update state and broadcast
        SignalMessage::Mute {
            ref participant_id,
            kind,
        } => {
            let (audio, video) = match kind {
                MuteKind::Audio => (Some(true), None),
                MuteKind::Video => (None, Some(true)),
            };
            let _ = state.store.set_mute(participant_id, audio, video);
            state.broadcast_to_room(room_id, msg);
        }
        SignalMessage::Unmute {
            ref participant_id,
            kind,
        } => {
            let (audio, video) = match kind {
                MuteKind::Audio => (Some(false), None),
                MuteKind::Video => (None, Some(false)),
            };
            let _ = state.store.set_mute(participant_id, audio, video);
            state.broadcast_to_room(room_id, msg);
        }

        SignalMessage::RaiseHand {
            ref participant_id,
        } => {
            let _ = state.store.toggle_hand(participant_id);
            state.broadcast_to_room(room_id, msg);
        }
        SignalMessage::LowerHand {
            ref participant_id,
        } => {
            let _ = state.store.toggle_hand(participant_id);
            state.broadcast_to_room(room_id, msg);
        }

        SignalMessage::ScreenShare {
            ref participant_id,
        } => {
            let _ = state.store.set_screen_sharing(participant_id, true);
            state.broadcast_to_room(room_id, msg);
        }
        SignalMessage::StopScreenShare {
            ref participant_id,
        } => {
            let _ = state.store.set_screen_sharing(participant_id, false);
            state.broadcast_to_room(room_id, msg);
        }

        SignalMessage::ChatMessage { .. } => {
            // Broadcast chat to all room participants
            state.broadcast_to_room(room_id, msg);
        }

        SignalMessage::Kick {
            ref participant_id, ..
        } => {
            // Only moderators can kick
            if state.store.can_moderate(&sender_pid) {
                let _ = state.store.leave_room(&room_id, participant_id);
                state.broadcast_to_room(room_id, msg);
            }
        }

        SignalMessage::EndRoom { room_id: rid } => {
            // Only moderators/host can end the room
            if state.store.can_moderate(&sender_pid) {
                state.broadcast_to_room(rid, msg);
                let _ = state.store.end_room(&rid);
                state.remove_room_channel(&rid);
            }
        }

        // Join/Leave are handled by the connection lifecycle, not client messages
        SignalMessage::Join { .. } | SignalMessage::Leave { .. } => {}
    }
}
