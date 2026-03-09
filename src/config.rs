//! Application configuration following the shikumi pattern.
//!
//! Configuration is loaded in priority order:
//!
//! 1. Environment variables (`TAIMEN_*`)
//! 2. Config file (`~/.config/taimen/config.toml`)
//! 3. Compiled-in defaults (see [`TaimenConfig::default`])

use serde::{Deserialize, Serialize};

/// Top-level configuration for the Taimen client.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaimenConfig {
    /// URL of the signaling / media server (e.g. `wss://meet.example.com`).
    #[serde(default = "default_server_url")]
    pub server_url: String,

    /// Display name shown to other participants.
    #[serde(default = "default_display_name")]
    pub display_name: String,

    /// Start with microphone muted.
    #[serde(default)]
    pub default_audio_muted: bool,

    /// Start with camera off.
    #[serde(default)]
    pub default_video_muted: bool,

    /// Preferred video resolution (e.g. "1280x720").
    #[serde(default = "default_video_resolution")]
    pub video_resolution: String,

    /// Theme name for the GPU TUI (e.g. "dark", "light", "pleme").
    #[serde(default = "default_theme")]
    pub theme: String,
}

impl Default for TaimenConfig {
    fn default() -> Self {
        Self {
            server_url: default_server_url(),
            display_name: default_display_name(),
            default_audio_muted: false,
            default_video_muted: false,
            video_resolution: default_video_resolution(),
            theme: default_theme(),
        }
    }
}

fn default_server_url() -> String {
    "ws://localhost:3000".into()
}

fn default_display_name() -> String {
    "Anonymous".into()
}

fn default_video_resolution() -> String {
    "1280x720".into()
}

fn default_theme() -> String {
    "dark".into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_values() {
        let cfg = TaimenConfig::default();
        assert_eq!(cfg.server_url, "ws://localhost:3000");
        assert_eq!(cfg.display_name, "Anonymous");
        assert!(!cfg.default_audio_muted);
        assert!(!cfg.default_video_muted);
        assert_eq!(cfg.video_resolution, "1280x720");
        assert_eq!(cfg.theme, "dark");
    }

    #[test]
    fn config_serde_roundtrip() {
        let cfg = TaimenConfig {
            server_url: "wss://meet.pleme.io".into(),
            display_name: "Taro".into(),
            default_audio_muted: true,
            default_video_muted: false,
            video_resolution: "1920x1080".into(),
            theme: "pleme".into(),
        };
        let json = serde_json::to_string(&cfg).unwrap();
        let restored: TaimenConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.server_url, "wss://meet.pleme.io");
        assert_eq!(restored.display_name, "Taro");
        assert!(restored.default_audio_muted);
        assert!(!restored.default_video_muted);
        assert_eq!(restored.video_resolution, "1920x1080");
        assert_eq!(restored.theme, "pleme");
    }

    #[test]
    fn config_deserializes_with_defaults() {
        let json = r#"{}"#;
        let cfg: TaimenConfig = serde_json::from_str(json).unwrap();
        assert_eq!(cfg.server_url, "ws://localhost:3000");
        assert_eq!(cfg.display_name, "Anonymous");
        assert_eq!(cfg.theme, "dark");
    }

    #[test]
    fn config_partial_override() {
        let json = r#"{"display_name": "Hanako", "theme": "light"}"#;
        let cfg: TaimenConfig = serde_json::from_str(json).unwrap();
        assert_eq!(cfg.display_name, "Hanako");
        assert_eq!(cfg.theme, "light");
        // defaults still apply for fields not provided
        assert_eq!(cfg.server_url, "ws://localhost:3000");
        assert_eq!(cfg.video_resolution, "1280x720");
    }
}
