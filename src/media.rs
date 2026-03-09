use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::participant::ParticipantId;

/// Unique identifier for a media track.
pub type TrackId = Uuid;

/// A single media track belonging to a participant.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaTrack {
    /// Unique track identifier.
    pub id: TrackId,
    /// What kind of media this track carries.
    pub kind: TrackKind,
    /// The participant who owns this track.
    pub participant_id: ParticipantId,
    /// Whether the track is currently sending data.
    pub enabled: bool,
}

impl MediaTrack {
    /// Create a new enabled media track.
    #[must_use]
    pub fn new(kind: TrackKind, participant_id: ParticipantId) -> Self {
        Self {
            id: Uuid::new_v4(),
            kind,
            participant_id,
            enabled: true,
        }
    }
}

/// The type of media carried by a track.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrackKind {
    /// Microphone audio.
    Audio,
    /// Camera video.
    Video,
    /// Screen-share stream (may contain both video and audio).
    ScreenShare,
}

/// Configuration for media encoding and quality.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaConfig {
    /// Target video resolution (e.g. "1280x720").
    pub video_resolution: String,
    /// Target video frames per second.
    pub video_fps: u32,
    /// Preferred audio codec (e.g. "opus").
    pub audio_codec: String,
    /// Preferred video codec (e.g. "vp9", "h264", "av1").
    pub video_codec: String,
}

impl Default for MediaConfig {
    fn default() -> Self {
        Self {
            video_resolution: "1280x720".into(),
            video_fps: 30,
            audio_codec: "opus".into(),
            video_codec: "vp9".into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_track_is_enabled() {
        let pid = Uuid::new_v4();
        let track = MediaTrack::new(TrackKind::Audio, pid);
        assert!(track.enabled);
        assert_eq!(track.kind, TrackKind::Audio);
        assert_eq!(track.participant_id, pid);
    }

    #[test]
    fn track_kind_serde() {
        for kind in [TrackKind::Audio, TrackKind::Video, TrackKind::ScreenShare] {
            let json = serde_json::to_string(&kind).unwrap();
            let restored: TrackKind = serde_json::from_str(&json).unwrap();
            assert_eq!(restored, kind);
        }
    }

    #[test]
    fn track_kind_json_values() {
        assert_eq!(
            serde_json::to_string(&TrackKind::Audio).unwrap(),
            "\"audio\""
        );
        assert_eq!(
            serde_json::to_string(&TrackKind::Video).unwrap(),
            "\"video\""
        );
        assert_eq!(
            serde_json::to_string(&TrackKind::ScreenShare).unwrap(),
            "\"screen_share\""
        );
    }

    #[test]
    fn media_track_roundtrip() {
        let track = MediaTrack::new(TrackKind::Video, Uuid::new_v4());
        let json = serde_json::to_string(&track).unwrap();
        let restored: MediaTrack = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.id, track.id);
        assert_eq!(restored.kind, TrackKind::Video);
        assert!(restored.enabled);
    }

    #[test]
    fn default_media_config() {
        let cfg = MediaConfig::default();
        assert_eq!(cfg.video_resolution, "1280x720");
        assert_eq!(cfg.video_fps, 30);
        assert_eq!(cfg.audio_codec, "opus");
        assert_eq!(cfg.video_codec, "vp9");
    }

    #[test]
    fn media_config_roundtrip() {
        let cfg = MediaConfig {
            video_resolution: "1920x1080".into(),
            video_fps: 60,
            audio_codec: "opus".into(),
            video_codec: "av1".into(),
        };
        let json = serde_json::to_string(&cfg).unwrap();
        let restored: MediaConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.video_resolution, "1920x1080");
        assert_eq!(restored.video_fps, 60);
        assert_eq!(restored.video_codec, "av1");
    }
}
