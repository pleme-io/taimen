//! MCP server for taimen video conferencing administration.
//!
//! Tools:
//!   `status`             — server health status
//!   `version`            — server version info
//!   `config_get`         — get a config value by key
//!   `config_set`         — set a config value
//!   `create_room`        — create a new meeting room
//!   `list_rooms`         — list active meeting rooms
//!   `get_participants`   — list participants in a room
//!   `end_room`           — end a meeting for all participants
//!   `get_stats`          — room or server statistics

use kaname::rmcp::{
    ServerHandler, ServiceExt,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{ServerCapabilities, ServerInfo},
    schemars, tool, tool_handler, tool_router,
    transport::stdio,
};
use serde::Deserialize;
use serde_json::json;

// ── Tool input types ────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct ConfigGetInput {
    #[schemars(description = "Config key to retrieve (e.g. 'server.listen', 'rooms.max_participants', 'recording.enabled').")]
    key: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct ConfigSetInput {
    #[schemars(description = "Config key to set.")]
    key: String,
    #[schemars(description = "New value as a JSON string.")]
    value: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct CreateRoomInput {
    #[schemars(description = "Maximum number of participants (default 100).")]
    max_participants: Option<usize>,
    #[schemars(description = "Enable waiting room (host must admit participants).")]
    enable_waiting_room: Option<bool>,
    #[schemars(description = "Enable server-side recording.")]
    enable_recording: Option<bool>,
    #[schemars(description = "Enable in-meeting chat.")]
    enable_chat: Option<bool>,
    #[schemars(description = "Room password (optional).")]
    password: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct ListRoomsInput {
    #[schemars(description = "Filter by room state: 'active', 'waiting', 'ended'. Omit for all active rooms.")]
    state: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct GetParticipantsInput {
    #[schemars(description = "Room ID to list participants for.")]
    room_id: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct EndRoomInput {
    #[schemars(description = "Room ID to end.")]
    room_id: String,
    #[schemars(description = "Reason for ending the meeting (sent to all participants).")]
    reason: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct GetStatsInput {
    #[schemars(description = "Room ID for room-specific stats. Omit for global server stats.")]
    room_id: Option<String>,
}

// ── MCP Server ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct TaimenMcp {
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl TaimenMcp {
    fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }

    // ── Standard tools ──────────────────────────────────────────────────────

    #[tool(description = "Get taimen server health status: uptime, active rooms, connected participants, bandwidth.")]
    async fn status(&self) -> String {
        // TODO: query AppState for live metrics
        serde_json::to_string(&json!({
            "healthy": true,
            "uptime_secs": 0,
            "active_rooms": 0,
            "total_participants": 0,
            "bandwidth_mbps": 0.0
        }))
        .unwrap_or_default()
    }

    #[tool(description = "Get taimen version information.")]
    async fn version(&self) -> String {
        serde_json::to_string(&json!({
            "name": "taimen",
            "version": env!("CARGO_PKG_VERSION"),
            "features": ["webrtc_sfu", "signaling", "recording", "screen_share"]
        }))
        .unwrap_or_default()
    }

    #[tool(description = "Get a server configuration value by key.")]
    async fn config_get(&self, Parameters(input): Parameters<ConfigGetInput>) -> String {
        // TODO: read from TaimenConfig
        serde_json::to_string(&json!({
            "key": input.key,
            "value": null,
            "error": "config not available in MCP context"
        }))
        .unwrap_or_default()
    }

    #[tool(description = "Set a server configuration value. Some changes require a server restart.")]
    async fn config_set(&self, Parameters(input): Parameters<ConfigSetInput>) -> String {
        // TODO: write to TaimenConfig
        serde_json::to_string(&json!({
            "key": input.key,
            "value": input.value,
            "applied": false,
            "error": "config changes not available in MCP context"
        }))
        .unwrap_or_default()
    }

    // ── Room management tools ───────────────────────────────────────────────

    #[tool(description = "Create a new meeting room. Returns room ID and join URL. Optionally configure max participants, waiting room, recording, and password.")]
    async fn create_room(&self, Parameters(input): Parameters<CreateRoomInput>) -> String {
        // TODO: create room in AppState
        serde_json::to_string(&json!({
            "ok": false,
            "room_id": null,
            "config": {
                "max_participants": input.max_participants.unwrap_or(100),
                "enable_waiting_room": input.enable_waiting_room.unwrap_or(false),
                "enable_recording": input.enable_recording.unwrap_or(false),
                "enable_chat": input.enable_chat.unwrap_or(true),
                "has_password": input.password.is_some()
            },
            "error": "room creation not available in MCP context"
        }))
        .unwrap_or_default()
    }

    #[tool(description = "List active meeting rooms with participant counts and duration.")]
    async fn list_rooms(&self, Parameters(input): Parameters<ListRoomsInput>) -> String {
        // TODO: query AppState for rooms
        serde_json::to_string(&json!({
            "state_filter": input.state,
            "rooms": [],
            "total": 0
        }))
        .unwrap_or_default()
    }

    #[tool(description = "List all participants in a meeting room with their roles, mute state, and connection quality.")]
    async fn get_participants(&self, Parameters(input): Parameters<GetParticipantsInput>) -> String {
        // TODO: query room for participants
        serde_json::to_string(&json!({
            "room_id": input.room_id,
            "participants": [],
            "total": 0
        }))
        .unwrap_or_default()
    }

    #[tool(description = "End a meeting room, disconnecting all participants. Optionally provide a reason.")]
    async fn end_room(&self, Parameters(input): Parameters<EndRoomInput>) -> String {
        // TODO: end room in AppState
        serde_json::to_string(&json!({
            "ok": false,
            "room_id": input.room_id,
            "reason": input.reason,
            "error": "room management not available in MCP context"
        }))
        .unwrap_or_default()
    }

    #[tool(description = "Get statistics for a specific room or the entire server: duration, participant count, bandwidth, recording status.")]
    async fn get_stats(&self, Parameters(input): Parameters<GetStatsInput>) -> String {
        // TODO: aggregate stats from AppState
        serde_json::to_string(&json!({
            "room_id": input.room_id,
            "active_rooms": 0,
            "total_participants": 0,
            "peak_participants": 0,
            "total_meetings_today": 0,
            "avg_duration_secs": 0,
            "bandwidth_mbps": 0.0,
            "recordings_active": 0
        }))
        .unwrap_or_default()
    }
}

#[tool_handler]
impl ServerHandler for TaimenMcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                "Taimen video conferencing server — room management, participant control, and server statistics."
                    .into(),
            ),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}

pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let server = TaimenMcp::new().serve(stdio()).await?;
    server.waiting().await?;
    Ok(())
}
