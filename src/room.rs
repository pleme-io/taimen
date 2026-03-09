use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::participant::ParticipantId;

/// Unique identifier for a room.
pub type RoomId = Uuid;

/// A video-conferencing room.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Room {
    /// Unique room identifier.
    pub id: RoomId,
    /// Human-readable room name.
    pub name: String,
    /// The participant who created / hosts the room.
    pub host_id: ParticipantId,
    /// Currently joined participants.
    pub participants: Vec<ParticipantId>,
    /// Maximum number of participants allowed.
    pub max_participants: usize,
    /// When the room was created.
    pub created_at: DateTime<Utc>,
    /// Room-level configuration.
    pub config: RoomConfig,
    /// Current lifecycle state.
    pub state: RoomState,
}

impl Room {
    /// Create a new room with the given name and host.
    #[must_use]
    pub fn new(name: impl Into<String>, host_id: ParticipantId, max_participants: usize) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            host_id,
            participants: vec![host_id],
            max_participants,
            created_at: Utc::now(),
            config: RoomConfig::default(),
            state: RoomState::Waiting,
        }
    }

    /// Returns `true` if the room has reached its capacity.
    #[must_use]
    pub fn is_full(&self) -> bool {
        self.participants.len() >= self.max_participants
    }

    /// Add a participant to the room.
    ///
    /// # Errors
    ///
    /// Returns an error if the room is at capacity.
    pub fn add_participant(&mut self, id: ParticipantId) -> crate::error::Result<()> {
        if self.is_full() {
            return Err(crate::TaimenError::Capacity(format!(
                "room {} is full ({}/{})",
                self.id,
                self.participants.len(),
                self.max_participants,
            )));
        }
        self.participants.push(id);
        Ok(())
    }

    /// Remove a participant from the room.
    pub fn remove_participant(&mut self, id: ParticipantId) {
        self.participants.retain(|p| *p != id);
    }
}

/// Room-level feature toggles.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomConfig {
    /// Whether video tracks are enabled.
    pub video_enabled: bool,
    /// Whether audio tracks are enabled.
    pub audio_enabled: bool,
    /// Whether screen-sharing is allowed.
    pub screen_share_enabled: bool,
    /// Whether the room may be recorded.
    pub recording_enabled: bool,
    /// Optional maximum duration for the room session.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "optional_duration_secs"
    )]
    pub max_duration: Option<Duration>,
}

impl Default for RoomConfig {
    fn default() -> Self {
        Self {
            video_enabled: true,
            audio_enabled: true,
            screen_share_enabled: true,
            recording_enabled: false,
            max_duration: None,
        }
    }
}

/// Lifecycle state of a room.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RoomState {
    /// Room exists but the meeting has not started.
    Waiting,
    /// Meeting is in progress.
    Active,
    /// Meeting has ended.
    Ended,
}

/// Serde helper for `Option<Duration>` stored as seconds.
mod optional_duration_secs {
    use std::time::Duration;

    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(val: &Option<Duration>, ser: S) -> Result<S::Ok, S::Error> {
        match val {
            Some(d) => ser.serialize_u64(d.as_secs()),
            None => ser.serialize_none(),
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(de: D) -> Result<Option<Duration>, D::Error> {
        let opt: Option<u64> = Option::deserialize(de)?;
        Ok(opt.map(Duration::from_secs))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_room_has_host_as_participant() {
        let host = Uuid::new_v4();
        let room = Room::new("Daily standup", host, 10);
        assert_eq!(room.participants.len(), 1);
        assert_eq!(room.participants[0], host);
        assert_eq!(room.state, RoomState::Waiting);
    }

    #[test]
    fn room_capacity_check() {
        let host = Uuid::new_v4();
        let mut room = Room::new("Small room", host, 2);
        assert!(!room.is_full());

        let p2 = Uuid::new_v4();
        room.add_participant(p2).unwrap();
        assert!(room.is_full());

        let p3 = Uuid::new_v4();
        assert!(room.add_participant(p3).is_err());
    }

    #[test]
    fn remove_participant() {
        let host = Uuid::new_v4();
        let mut room = Room::new("Test", host, 10);
        let p2 = Uuid::new_v4();
        room.add_participant(p2).unwrap();
        assert_eq!(room.participants.len(), 2);

        room.remove_participant(p2);
        assert_eq!(room.participants.len(), 1);
    }

    #[test]
    fn default_room_config() {
        let cfg = RoomConfig::default();
        assert!(cfg.video_enabled);
        assert!(cfg.audio_enabled);
        assert!(cfg.screen_share_enabled);
        assert!(!cfg.recording_enabled);
        assert!(cfg.max_duration.is_none());
    }

    #[test]
    fn room_config_roundtrip() {
        let cfg = RoomConfig {
            max_duration: Some(Duration::from_secs(3600)),
            ..RoomConfig::default()
        };
        let json = serde_json::to_string(&cfg).unwrap();
        let restored: RoomConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.max_duration, Some(Duration::from_secs(3600)));
    }

    #[test]
    fn room_state_serde() {
        let json = serde_json::to_string(&RoomState::Active).unwrap();
        let restored: RoomState = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, RoomState::Active);
    }
}
