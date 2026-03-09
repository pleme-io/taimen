//! Shared application state for the Taimen signaling server.

use std::sync::Arc;

use dashmap::DashMap;
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::signal::SignalMessage;
use crate::storage::Store;

/// Unique identifier for a WebSocket connection.
pub type ConnectionId = Uuid;

/// Application state shared across all request handlers and WebSocket connections.
#[derive(Clone)]
pub struct AppState {
    /// In-memory data store.
    pub store: Store,
    /// JWT secret for room tokens.
    pub jwt_secret: Arc<String>,
    /// Per-room broadcast channels for signaling messages.
    pub room_channels: Arc<DashMap<Uuid, broadcast::Sender<SignalMessage>>>,
}

impl AppState {
    /// Create a new application state.
    #[must_use]
    pub fn new(jwt_secret: impl Into<String>) -> Self {
        Self {
            store: Store::new(),
            jwt_secret: Arc::new(jwt_secret.into()),
            room_channels: Arc::new(DashMap::new()),
        }
    }

    /// Get or create a broadcast channel for a room.
    pub fn room_sender(&self, room_id: Uuid) -> broadcast::Sender<SignalMessage> {
        self.room_channels
            .entry(room_id)
            .or_insert_with(|| broadcast::channel(256).0)
            .clone()
    }

    /// Subscribe to a room's broadcast channel.
    pub fn room_subscribe(&self, room_id: Uuid) -> broadcast::Receiver<SignalMessage> {
        self.room_sender(room_id).subscribe()
    }

    /// Broadcast a signal message to all participants in a room.
    pub fn broadcast_to_room(&self, room_id: Uuid, msg: SignalMessage) {
        let _ = self.room_sender(room_id).send(msg);
    }

    /// Remove a room's broadcast channel (on room end).
    pub fn remove_room_channel(&self, room_id: &Uuid) {
        self.room_channels.remove(room_id);
    }
}
