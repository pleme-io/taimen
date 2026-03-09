use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Unique identifier for a participant.
pub type ParticipantId = Uuid;

/// A user's identity and state within a room.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Participant {
    /// Unique participant identifier (assigned on join).
    pub id: ParticipantId,
    /// Stable user identifier (may equal `id` for guests).
    pub user_id: Uuid,
    /// Display name shown in the UI.
    pub display_name: String,
    /// The participant's role in the room.
    pub role: ParticipantRole,
    /// Whether the participant's microphone is muted.
    pub audio_muted: bool,
    /// Whether the participant's camera is off.
    pub video_muted: bool,
    /// Whether the participant is sharing their screen.
    pub screen_sharing: bool,
    /// Whether the participant has their hand raised.
    pub hand_raised: bool,
    /// When the participant joined the room.
    pub joined_at: DateTime<Utc>,
}

impl Participant {
    /// Create a new participant with sensible defaults.
    #[must_use]
    pub fn new(user_id: Uuid, display_name: impl Into<String>, role: ParticipantRole) -> Self {
        Self {
            id: Uuid::new_v4(),
            user_id,
            display_name: display_name.into(),
            role,
            audio_muted: false,
            video_muted: false,
            screen_sharing: false,
            hand_raised: false,
            joined_at: Utc::now(),
        }
    }

    /// Toggle the hand-raised state and return the new value.
    pub fn toggle_hand(&mut self) -> bool {
        self.hand_raised = !self.hand_raised;
        self.hand_raised
    }
}

/// A participant's role within a room, ordered by privilege.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ParticipantRole {
    /// Created the room; full control.
    Host,
    /// Elevated permissions (mute others, kick).
    Moderator,
    /// Standard participant with audio/video.
    Participant,
    /// View-only; cannot send media.
    Viewer,
}

impl ParticipantRole {
    /// Returns `true` if the role can mute or kick other participants.
    #[must_use]
    pub const fn can_moderate(&self) -> bool {
        matches!(self, Self::Host | Self::Moderator)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_participant_defaults() {
        let uid = Uuid::new_v4();
        let p = Participant::new(uid, "Alice", ParticipantRole::Participant);
        assert_eq!(p.user_id, uid);
        assert_eq!(p.display_name, "Alice");
        assert!(!p.audio_muted);
        assert!(!p.video_muted);
        assert!(!p.screen_sharing);
        assert!(!p.hand_raised);
    }

    #[test]
    fn toggle_hand() {
        let mut p = Participant::new(Uuid::new_v4(), "Bob", ParticipantRole::Participant);
        assert!(!p.hand_raised);
        assert!(p.toggle_hand());
        assert!(p.hand_raised);
        assert!(!p.toggle_hand());
        assert!(!p.hand_raised);
    }

    #[test]
    fn role_moderation_privileges() {
        assert!(ParticipantRole::Host.can_moderate());
        assert!(ParticipantRole::Moderator.can_moderate());
        assert!(!ParticipantRole::Participant.can_moderate());
        assert!(!ParticipantRole::Viewer.can_moderate());
    }

    #[test]
    fn participant_serde_roundtrip() {
        let p = Participant::new(Uuid::new_v4(), "Carol", ParticipantRole::Moderator);
        let json = serde_json::to_string(&p).unwrap();
        let restored: Participant = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.display_name, "Carol");
        assert_eq!(restored.role, ParticipantRole::Moderator);
    }

    #[test]
    fn role_serde() {
        for role in [
            ParticipantRole::Host,
            ParticipantRole::Moderator,
            ParticipantRole::Participant,
            ParticipantRole::Viewer,
        ] {
            let json = serde_json::to_string(&role).unwrap();
            let restored: ParticipantRole = serde_json::from_str(&json).unwrap();
            assert_eq!(restored, role);
        }
    }
}
