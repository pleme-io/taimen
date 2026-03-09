//! WebSocket signaling protocol for Taimen.
//!
//! # Protocol overview
//!
//! Clients connect to the signaling server over a WebSocket at `/ws/{room_id}`.
//! Every message on the wire is a JSON-serialised [`SignalMessage`].
//!
//! ## Connection lifecycle
//!
//! 1. Client sends [`SignalMessage::Join`] with its participant info.
//! 2. Server broadcasts the join event to all other participants.
//! 3. Existing participants send [`SignalMessage::Offer`] to the new peer.
//! 4. New peer responds with [`SignalMessage::Answer`] to each offer.
//! 5. Both sides exchange [`SignalMessage::IceCandidate`] messages until the
//!    ICE connection is established.
//! 6. Participants may send control messages (mute, raise hand, etc.) at any time.
//! 7. When a participant disconnects, the server broadcasts [`SignalMessage::Leave`].
//!
//! ## Room lifecycle
//!
//! The host (or a moderator) may send [`SignalMessage::EndRoom`] to terminate
//! the session for all participants.  The server sends `EndRoom` to every
//! connected client and then tears down the room state.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::participant::ParticipantId;
use crate::room::RoomId;

/// A message exchanged over the signaling WebSocket.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SignalMessage {
    /// A participant wants to join the room.
    Join {
        room_id: RoomId,
        participant_id: ParticipantId,
        display_name: String,
    },

    /// A participant has left (or been disconnected from) the room.
    Leave {
        room_id: RoomId,
        participant_id: ParticipantId,
    },

    /// SDP offer for WebRTC negotiation.
    Offer {
        from: ParticipantId,
        to: ParticipantId,
        sdp: String,
    },

    /// SDP answer for WebRTC negotiation.
    Answer {
        from: ParticipantId,
        to: ParticipantId,
        sdp: String,
    },

    /// An ICE candidate for connection establishment.
    IceCandidate {
        from: ParticipantId,
        to: ParticipantId,
        candidate: String,
        sdp_mid: Option<String>,
        sdp_m_line_index: Option<u32>,
    },

    /// Participant muted their audio or video.
    Mute {
        participant_id: ParticipantId,
        kind: MuteKind,
    },

    /// Participant unmuted their audio or video.
    Unmute {
        participant_id: ParticipantId,
        kind: MuteKind,
    },

    /// Participant raised their hand.
    RaiseHand { participant_id: ParticipantId },

    /// Participant lowered their hand.
    LowerHand { participant_id: ParticipantId },

    /// Participant started screen-sharing.
    ScreenShare { participant_id: ParticipantId },

    /// Participant stopped screen-sharing.
    StopScreenShare { participant_id: ParticipantId },

    /// An in-room chat message.
    ChatMessage {
        id: Uuid,
        participant_id: ParticipantId,
        content: String,
        timestamp: chrono::DateTime<chrono::Utc>,
    },

    /// A moderator/host kicks a participant from the room.
    Kick {
        participant_id: ParticipantId,
        reason: Option<String>,
    },

    /// The host/moderator ends the room for everyone.
    EndRoom { room_id: RoomId },
}

/// Distinguishes audio and video for mute/unmute messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MuteKind {
    Audio,
    Video,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn join_message_roundtrip() {
        let msg = SignalMessage::Join {
            room_id: Uuid::new_v4(),
            participant_id: Uuid::new_v4(),
            display_name: "Alice".into(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"join\""));
        let restored: SignalMessage = serde_json::from_str(&json).unwrap();
        match restored {
            SignalMessage::Join { display_name, .. } => assert_eq!(display_name, "Alice"),
            _ => panic!("expected Join variant"),
        }
    }

    #[test]
    fn offer_answer_roundtrip() {
        let from = Uuid::new_v4();
        let to = Uuid::new_v4();
        let offer = SignalMessage::Offer {
            from,
            to,
            sdp: "v=0\r\n...".into(),
        };
        let json = serde_json::to_string(&offer).unwrap();
        let restored: SignalMessage = serde_json::from_str(&json).unwrap();
        match restored {
            SignalMessage::Offer { sdp, .. } => assert!(sdp.starts_with("v=0")),
            _ => panic!("expected Offer variant"),
        }
    }

    #[test]
    fn ice_candidate_roundtrip() {
        let msg = SignalMessage::IceCandidate {
            from: Uuid::new_v4(),
            to: Uuid::new_v4(),
            candidate: "candidate:1 1 UDP 2130706431 ...".into(),
            sdp_mid: Some("0".into()),
            sdp_m_line_index: Some(0),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let restored: SignalMessage = serde_json::from_str(&json).unwrap();
        match restored {
            SignalMessage::IceCandidate { candidate, .. } => {
                assert!(candidate.starts_with("candidate:"));
            }
            _ => panic!("expected IceCandidate variant"),
        }
    }

    #[test]
    fn mute_unmute_roundtrip() {
        let pid = Uuid::new_v4();
        let msg = SignalMessage::Mute {
            participant_id: pid,
            kind: MuteKind::Audio,
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"audio\""));
        let restored: SignalMessage = serde_json::from_str(&json).unwrap();
        match restored {
            SignalMessage::Mute { kind, .. } => assert_eq!(kind, MuteKind::Audio),
            _ => panic!("expected Mute variant"),
        }
    }

    #[test]
    fn chat_message_roundtrip() {
        let msg = SignalMessage::ChatMessage {
            id: Uuid::new_v4(),
            participant_id: Uuid::new_v4(),
            content: "Hello, world!".into(),
            timestamp: chrono::Utc::now(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("Hello, world!"));
    }

    #[test]
    fn kick_message_roundtrip() {
        let msg = SignalMessage::Kick {
            participant_id: Uuid::new_v4(),
            reason: Some("disruptive".into()),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let restored: SignalMessage = serde_json::from_str(&json).unwrap();
        match restored {
            SignalMessage::Kick { reason, .. } => assert_eq!(reason.as_deref(), Some("disruptive")),
            _ => panic!("expected Kick variant"),
        }
    }

    #[test]
    fn end_room_roundtrip() {
        let rid = Uuid::new_v4();
        let msg = SignalMessage::EndRoom { room_id: rid };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"end_room\""));
    }

    #[test]
    fn all_control_variants_serialize() {
        let pid = Uuid::new_v4();
        let messages = vec![
            SignalMessage::RaiseHand {
                participant_id: pid,
            },
            SignalMessage::LowerHand {
                participant_id: pid,
            },
            SignalMessage::ScreenShare {
                participant_id: pid,
            },
            SignalMessage::StopScreenShare {
                participant_id: pid,
            },
        ];
        for msg in messages {
            let json = serde_json::to_string(&msg).unwrap();
            let _: SignalMessage = serde_json::from_str(&json).unwrap();
        }
    }
}
