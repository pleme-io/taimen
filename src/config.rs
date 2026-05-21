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

impl shikumi::TieredConfig for TaimenConfig {
    /// Tier 0 — bare: zero-opinion floor. Empty server URL, empty
    /// display name, empty video resolution, empty theme — documents
    /// the minimum that won't try to connect anywhere or render
    /// without an explicit operator choice.
    fn bare() -> Self {
        Self {
            server_url: String::new(),
            display_name: String::new(),
            default_audio_muted: false,
            default_video_muted: false,
            video_resolution: String::new(),
            theme: String::new(),
        }
    }

    /// Tier 2 — prescribed: the curated taimen defaults shipped today
    /// (ws://localhost:3000, "Anonymous", 1280x720, "dark"). Delegates
    /// to Default for the single-source-of-truth invariant.
    fn prescribed_default() -> Self {
        Self::default()
    }
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

#[cfg(test)]
mod tiered_tests {
    use super::*;
    use shikumi::{ConfigTier, TieredConfig};

    #[test]
    fn bare_is_zero_opinion() {
        let b = <TaimenConfig as TieredConfig>::bare();
        assert_eq!(b.server_url, "");
        assert_eq!(b.display_name, "");
        assert_eq!(b.video_resolution, "");
        assert_eq!(b.theme, "");
        assert!(!b.default_audio_muted);
        assert!(!b.default_video_muted);
    }

    #[test]
    fn prescribed_matches_default() {
        let p = <TaimenConfig as TieredConfig>::prescribed_default();
        let d = TaimenConfig::default();
        assert_eq!(p.server_url, d.server_url);
        assert_eq!(p.theme, d.theme);
    }

    #[test]
    fn diff_bare_vs_default_is_non_empty() {
        let b = <TaimenConfig as TieredConfig>::bare();
        let d = <TaimenConfig as TieredConfig>::prescribed_default();
        let diff = d.diff_against(&b);
        assert!(
            !diff.is_empty_diff(),
            "bare and prescribed_default must differ"
        );
    }

    #[test]
    fn resolve_tier_dispatches() {
        assert_eq!(
            <TaimenConfig as TieredConfig>::resolve_tier(ConfigTier::Bare).server_url,
            ""
        );
        assert_eq!(
            <TaimenConfig as TieredConfig>::resolve_tier(ConfigTier::Default).server_url,
            "ws://localhost:3000"
        );
    }
}
