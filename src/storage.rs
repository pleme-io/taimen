//! In-memory storage for rooms, participants, and recording metadata.

use std::sync::Arc;

use chrono::Utc;
use dashmap::DashMap;
use uuid::Uuid;

use crate::error::{Result, TaimenError};
use crate::participant::{Participant, ParticipantId, ParticipantRole};
use crate::room::{Room, RoomId, RoomState};

/// Shared application state for the signaling server.
#[derive(Clone)]
pub struct Store {
    inner: Arc<StoreInner>,
}

struct StoreInner {
    /// Rooms keyed by room ID.
    rooms: DashMap<RoomId, Room>,
    /// Participants keyed by participant ID.
    participants: DashMap<ParticipantId, Participant>,
    /// Room ID -> list of participant IDs.
    room_participants: DashMap<RoomId, Vec<ParticipantId>>,
    /// Recording state: room ID -> is_recording.
    recording_state: DashMap<RoomId, RecordingInfo>,
}

/// Metadata about an active recording.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RecordingInfo {
    /// Whether recording is active.
    pub active: bool,
    /// When recording started.
    pub started_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Who started the recording.
    pub started_by: Option<ParticipantId>,
}

impl Store {
    /// Create a new empty store.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(StoreInner {
                rooms: DashMap::new(),
                participants: DashMap::new(),
                room_participants: DashMap::new(),
                recording_state: DashMap::new(),
            }),
        }
    }

    // ── Rooms ──

    /// Create a new room.
    #[must_use]
    pub fn create_room(
        &self,
        name: &str,
        host_id: ParticipantId,
        max_participants: usize,
    ) -> Room {
        let room = Room::new(name, host_id, max_participants);
        self.inner.rooms.insert(room.id, room.clone());
        self.inner.room_participants.insert(room.id, vec![host_id]);
        room
    }

    /// Get a room by ID.
    #[must_use]
    pub fn get_room(&self, room_id: &RoomId) -> Option<Room> {
        self.inner.rooms.get(room_id).map(|r| r.value().clone())
    }

    /// List all active rooms (not ended).
    #[must_use]
    pub fn list_rooms(&self) -> Vec<Room> {
        self.inner
            .rooms
            .iter()
            .filter(|r| r.value().state != RoomState::Ended)
            .map(|r| r.value().clone())
            .collect()
    }

    /// Update room state.
    pub fn set_room_state(&self, room_id: &RoomId, state: RoomState) -> Result<Room> {
        let mut entry = self
            .inner
            .rooms
            .get_mut(room_id)
            .ok_or_else(|| TaimenError::Room(format!("room {room_id} not found")))?;
        entry.state = state;
        Ok(entry.value().clone())
    }

    /// End a room -- sets state to Ended and removes all participants.
    pub fn end_room(&self, room_id: &RoomId) -> Result<()> {
        let mut entry = self
            .inner
            .rooms
            .get_mut(room_id)
            .ok_or_else(|| TaimenError::Room(format!("room {room_id} not found")))?;
        entry.state = RoomState::Ended;
        entry.participants.clear();
        drop(entry);

        // Clean up participant records
        if let Some((_, pids)) = self.inner.room_participants.remove(room_id) {
            for pid in pids {
                self.inner.participants.remove(&pid);
            }
        }
        self.inner.recording_state.remove(room_id);

        Ok(())
    }

    /// Delete a room completely.
    pub fn delete_room(&self, room_id: &RoomId) -> Result<()> {
        self.end_room(room_id)?;
        self.inner.rooms.remove(room_id);
        Ok(())
    }

    // ── Participants ──

    /// Add a participant to a room.
    pub fn join_room(
        &self,
        room_id: &RoomId,
        user_id: Uuid,
        display_name: &str,
        role: ParticipantRole,
    ) -> Result<Participant> {
        let mut room = self
            .inner
            .rooms
            .get_mut(room_id)
            .ok_or_else(|| TaimenError::Room(format!("room {room_id} not found")))?;

        if room.state == RoomState::Ended {
            return Err(TaimenError::Room("room has ended".into()));
        }

        let participant = Participant::new(user_id, display_name, role);

        room.add_participant(participant.id)?;
        drop(room);

        self.inner.participants.insert(participant.id, participant.clone());
        self.inner
            .room_participants
            .entry(*room_id)
            .or_default()
            .push(participant.id);

        // Transition room to Active if first join after Waiting
        if let Some(mut room_entry) = self.inner.rooms.get_mut(room_id) {
            if room_entry.state == RoomState::Waiting && room_entry.participants.len() > 1 {
                room_entry.state = RoomState::Active;
            }
        }

        Ok(participant)
    }

    /// Remove a participant from a room.
    pub fn leave_room(&self, room_id: &RoomId, participant_id: &ParticipantId) -> Result<()> {
        let mut room = self
            .inner
            .rooms
            .get_mut(room_id)
            .ok_or_else(|| TaimenError::Room(format!("room {room_id} not found")))?;

        room.remove_participant(*participant_id);
        drop(room);

        self.inner.participants.remove(participant_id);
        if let Some(mut pids) = self.inner.room_participants.get_mut(room_id) {
            pids.retain(|id| id != participant_id);
        }

        // End room if empty (no participants left)
        if let Some(room_entry) = self.inner.rooms.get(room_id) {
            if room_entry.participants.is_empty() {
                drop(room_entry);
                let _ = self.set_room_state(room_id, RoomState::Ended);
            }
        }

        Ok(())
    }

    /// Get a participant by ID.
    #[must_use]
    pub fn get_participant(&self, participant_id: &ParticipantId) -> Option<Participant> {
        self.inner
            .participants
            .get(participant_id)
            .map(|r| r.value().clone())
    }

    /// List participants in a room.
    #[must_use]
    pub fn list_participants(&self, room_id: &RoomId) -> Vec<Participant> {
        self.inner
            .room_participants
            .get(room_id)
            .map(|pids| {
                pids.value()
                    .iter()
                    .filter_map(|pid| self.get_participant(pid))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Update participant mute state.
    pub fn set_mute(
        &self,
        participant_id: &ParticipantId,
        audio_muted: Option<bool>,
        video_muted: Option<bool>,
    ) -> Result<Participant> {
        let mut entry = self
            .inner
            .participants
            .get_mut(participant_id)
            .ok_or_else(|| TaimenError::Signal(format!("participant {participant_id} not found")))?;
        if let Some(muted) = audio_muted {
            entry.audio_muted = muted;
        }
        if let Some(muted) = video_muted {
            entry.video_muted = muted;
        }
        Ok(entry.value().clone())
    }

    /// Toggle hand raise for a participant.
    pub fn toggle_hand(&self, participant_id: &ParticipantId) -> Result<bool> {
        let mut entry = self
            .inner
            .participants
            .get_mut(participant_id)
            .ok_or_else(|| TaimenError::Signal(format!("participant {participant_id} not found")))?;
        Ok(entry.toggle_hand())
    }

    /// Set screen sharing state.
    pub fn set_screen_sharing(
        &self,
        participant_id: &ParticipantId,
        sharing: bool,
    ) -> Result<Participant> {
        let mut entry = self
            .inner
            .participants
            .get_mut(participant_id)
            .ok_or_else(|| TaimenError::Signal(format!("participant {participant_id} not found")))?;
        entry.screen_sharing = sharing;
        Ok(entry.value().clone())
    }

    /// Check if a participant has moderator privileges.
    #[must_use]
    pub fn can_moderate(&self, participant_id: &ParticipantId) -> bool {
        self.inner
            .participants
            .get(participant_id)
            .is_some_and(|p| p.role.can_moderate())
    }

    // ── Recording ──

    /// Start recording for a room.
    pub fn start_recording(
        &self,
        room_id: &RoomId,
        started_by: ParticipantId,
    ) -> Result<RecordingInfo> {
        if self.get_room(room_id).is_none() {
            return Err(TaimenError::Room(format!("room {room_id} not found")));
        }
        let info = RecordingInfo {
            active: true,
            started_at: Some(Utc::now()),
            started_by: Some(started_by),
        };
        self.inner.recording_state.insert(*room_id, info.clone());
        Ok(info)
    }

    /// Stop recording for a room.
    pub fn stop_recording(&self, room_id: &RoomId) -> Result<RecordingInfo> {
        let mut entry = self
            .inner
            .recording_state
            .get_mut(room_id)
            .ok_or_else(|| TaimenError::Room("no active recording".into()))?;
        entry.active = false;
        Ok(entry.value().clone())
    }

    /// Get recording state for a room.
    #[must_use]
    pub fn get_recording_state(&self, room_id: &RoomId) -> Option<RecordingInfo> {
        self.inner
            .recording_state
            .get(room_id)
            .map(|r| r.value().clone())
    }
}

impl Default for Store {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_and_get_room() {
        let store = Store::new();
        let host = Uuid::new_v4();
        let room = store.create_room("Test Room", host, 10);
        assert_eq!(room.name, "Test Room");
        assert_eq!(room.host_id, host);
        assert_eq!(room.state, RoomState::Waiting);

        let found = store.get_room(&room.id).unwrap();
        assert_eq!(found.id, room.id);
    }

    #[test]
    fn list_rooms_excludes_ended() {
        let store = Store::new();
        let host = Uuid::new_v4();
        let r1 = store.create_room("Active", host, 10);
        let r2 = store.create_room("Ended", host, 10);
        store.set_room_state(&r2.id, RoomState::Ended).unwrap();

        let rooms = store.list_rooms();
        assert_eq!(rooms.len(), 1);
        assert_eq!(rooms[0].id, r1.id);
    }

    #[test]
    fn join_and_leave_room() {
        let store = Store::new();
        let host = Uuid::new_v4();
        let room = store.create_room("Test", host, 10);

        let user2 = Uuid::new_v4();
        let p = store
            .join_room(&room.id, user2, "Bob", ParticipantRole::Participant)
            .unwrap();
        assert_eq!(p.display_name, "Bob");

        let participants = store.list_participants(&room.id);
        assert_eq!(participants.len(), 1); // Only Bob in participants map (host not added via join_room)

        store.leave_room(&room.id, &p.id).unwrap();
        let participants = store.list_participants(&room.id);
        assert!(participants.is_empty());
    }

    #[test]
    fn room_capacity_enforced() {
        let store = Store::new();
        let host = Uuid::new_v4();
        let room = store.create_room("Small", host, 2);

        let u2 = Uuid::new_v4();
        store
            .join_room(&room.id, u2, "User2", ParticipantRole::Participant)
            .unwrap();

        let u3 = Uuid::new_v4();
        let result = store.join_room(&room.id, u3, "User3", ParticipantRole::Participant);
        assert!(result.is_err());
    }

    #[test]
    fn end_room_cleans_up() {
        let store = Store::new();
        let host = Uuid::new_v4();
        let room = store.create_room("Test", host, 10);

        let u2 = Uuid::new_v4();
        let p = store
            .join_room(&room.id, u2, "Bob", ParticipantRole::Participant)
            .unwrap();

        store.end_room(&room.id).unwrap();

        let ended_room = store.get_room(&room.id).unwrap();
        assert_eq!(ended_room.state, RoomState::Ended);
        assert!(store.get_participant(&p.id).is_none());
    }

    #[test]
    fn mute_and_screen_share() {
        let store = Store::new();
        let host = Uuid::new_v4();
        let room = store.create_room("Test", host, 10);
        let u = Uuid::new_v4();
        let p = store
            .join_room(&room.id, u, "Alice", ParticipantRole::Participant)
            .unwrap();

        let updated = store.set_mute(&p.id, Some(true), None).unwrap();
        assert!(updated.audio_muted);
        assert!(!updated.video_muted);

        let updated = store.set_screen_sharing(&p.id, true).unwrap();
        assert!(updated.screen_sharing);
    }

    #[test]
    fn recording_lifecycle() {
        let store = Store::new();
        let host = Uuid::new_v4();
        let room = store.create_room("Test", host, 10);

        let info = store.start_recording(&room.id, host).unwrap();
        assert!(info.active);

        let state = store.get_recording_state(&room.id).unwrap();
        assert!(state.active);

        let info = store.stop_recording(&room.id).unwrap();
        assert!(!info.active);
    }

    #[test]
    fn can_moderate_check() {
        let store = Store::new();
        let host = Uuid::new_v4();
        let room = store.create_room("Test", host, 10);

        let u1 = Uuid::new_v4();
        let mod_p = store
            .join_room(&room.id, u1, "Mod", ParticipantRole::Moderator)
            .unwrap();
        assert!(store.can_moderate(&mod_p.id));

        let u2 = Uuid::new_v4();
        let viewer = store
            .join_room(&room.id, u2, "Viewer", ParticipantRole::Viewer)
            .unwrap();
        assert!(!store.can_moderate(&viewer.id));
    }
}
