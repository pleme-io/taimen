# Taimen (対面) -- Video Conferencing Server

Open-source, self-hostable video conferencing platform. WebRTC SFU architecture
with axum signaling server. The pleme-io alternative to Zoom/Jitsi/LiveKit.

**This is primarily a SERVER application** with an optional GPU TUI client.
The server uses axum + denshin + eizou. The client uses garasu + madori + egaku.

## Build & Test

```bash
cargo build
cargo run -- server                # start signaling server
cargo run -- client <room_url>     # launch GPU TUI client
cargo run -- mcp                   # MCP admin server (stdio)
cargo test                         # unit tests
RUST_LOG=debug cargo run -- server # with tracing
```

Nix: `nix build`, `nix run .#server`, `nix run .#client`, `nix run .#container`

## Competitive Position

| Competitor | Weakness taimen addresses |
|-----------|--------------------------|
| Jitsi Meet (Java/JS) | Pure Rust -- lower latency, less memory, simpler deployment |
| LiveKit (Go) | Simpler deployment, pleme-io integration, Rhai scripting |
| BigBlueButton (Java) | Lighter, general-purpose (not education-specific), GPU TUI client |
| Galene (Go) | Richer feature set, MCP automation, Rhai scripting |
| La Suite Meet (Jitsi fork) | Self-hostable without government infrastructure dependencies |

Unique: MCP automation (AI meeting assistant), Rhai scripting for custom
meeting flows, GPU TUI client (not browser-only), pleme-io ecosystem integration.

## Architecture

### System Overview

```
                       +------------------+
GPU TUI Client ------->|                  |
  (garasu/madori)      |   taimen server  |------> STUN/TURN
                       |                  |
Web Client ----------->|  +-----------+   |------> Recording Storage
  (browser WebRTC)     |  | Signaling |   |         (S3/MinIO)
                       |  | (denshin) |   |
MCP Client ----------->|  +-----------+   |
  (kaname/stdio)       |  |    SFU    |   |
                       |  |  (eizou)  |   |
                       |  +-----------+   |
                       +------------------+
```

### Module Map -- Server

| Module | Purpose | Key Types |
|--------|---------|-----------|
| `room.rs` | Room lifecycle + state machine | `Room`, `RoomConfig`, `RoomState`, `RoomId` |
| `participant.rs` | Participant identity + state | `Participant`, `ParticipantRole`, `ParticipantId` |
| `signal.rs` | WebSocket signaling protocol | `SignalMessage` (14 variants) |
| `media.rs` | Media track configuration | `MediaTrack`, `MediaConfig`, `TrackKind` |
| `config.rs` | Server configuration (shikumi) | `TaimenConfig` |
| `error.rs` | Error types | `TaimenError`, `Result` |

### Target Module Map (planned expansion)

| Module | Purpose | Key Types |
|--------|---------|-----------|
| `server/` | axum HTTP server | Routes, middleware, CORS |
| `server/api.rs` | REST API routes | Room CRUD, participant management |
| `server/ws.rs` | WebSocket handler | Per-connection state, message dispatch |
| `signaling/` | Signaling logic (denshin) | Connection manager, room routing |
| `signaling/handler.rs` | Per-connection handler | SDP relay, ICE forwarding |
| `signaling/events.rs` | Event types | `SignalMessage`, serialization |
| `sfu/` | Media relay (eizou) | Track router, simulcast, bandwidth |
| `sfu/router.rs` | Track forwarding | Publisher -> subscriber routing |
| `sfu/quality.rs` | Quality adaptation | Bandwidth estimation, layer selection |
| `auth/` | Meeting auth (kenshou) | Room tokens, passwords, waiting rooms |
| `recording/` | Server-side recording | Composite recorder, S3 upload |
| `mcp.rs` | MCP admin server (kaname) | Room management, monitoring tools |
| `scripting/` | Rhai automation (soushi) | Meeting flow scripting |

### Target Module Map -- GPU TUI Client

| Module | Purpose | Key Types |
|--------|---------|-----------|
| `client/` | GPU TUI client | App state, render loop |
| `client/render.rs` | GPU rendering (garasu) | Participant grid, controls |
| `client/webrtc.rs` | WebRTC peer connection | Media tracks, ICE agent |
| `client/controls.rs` | Meeting controls (egaku) | Mute, camera, share, chat |
| `client/chat.rs` | In-meeting chat (egaku) | Message list, input |

### Signaling Protocol

WebSocket endpoint: `ws://<host>/ws/{room_id}`

The `SignalMessage` enum defines the wire protocol (14 variants):

**Connection lifecycle:**
1. Client sends `Join { room_id, participant_id, display_name }`
2. Server broadcasts `Join` to existing participants
3. Existing participants send `Offer { from, to, sdp }` to new peer
4. New peer responds with `Answer { from, to, sdp }` to each offer
5. Both sides exchange `IceCandidate { from, to, candidate, sdp_mid, sdp_m_line_index }`
6. ICE connection established, media flows peer-to-peer (via SFU)
7. On disconnect, server broadcasts `Leave { room_id, participant_id }`

**Control messages:**
- `Mute { participant_id, kind: Audio|Video }` / `Unmute`
- `RaiseHand { participant_id }` / `LowerHand`
- `ScreenShare { participant_id }` / `StopScreenShare`
- `ChatMessage { id, participant_id, content, timestamp }`
- `Kick { participant_id, reason }` -- moderator action
- `EndRoom { room_id }` -- host ends meeting for everyone

### SFU Architecture (eizou)

Selective Forwarding Unit -- receives media tracks from each participant and
selectively forwards them to others without transcoding.

```
Publisher A --[video track]--> SFU --[forward]--> Subscriber B
            --[audio track]-->     --[forward]--> Subscriber C
            --[screen share]-->    --[forward]--> Subscriber D

Publisher B --[video track]--> SFU --[forward]--> Subscriber A
            --[audio track]-->     --[forward]--> Subscriber C
```

**Why SFU (not mesh or MCU):**
- Mesh (P2P): Each participant sends to every other participant. Upload
  bandwidth grows linearly with participants. Does not scale beyond 3-4.
- MCU: Server decodes, mixes, and re-encodes all streams. CPU-intensive,
  adds latency, requires server-side codec support.
- SFU: Server forwards packets without processing. Low latency, low CPU,
  scales to 50+ participants. Each participant uploads once, downloads N-1 times.

**Simulcast:**
Each publisher sends multiple quality layers (e.g., 720p + 360p + 180p).
The SFU selects which layer to forward based on subscriber bandwidth and
viewing size. Subscribers with low bandwidth receive low-resolution tracks
automatically.

**Bandwidth estimation:**
Server-side REMB (Receiver Estimated Maximum Bitrate) or transport-wide
congestion control (TWCC). Adjusts forwarded layers dynamically.

### Room State Machine

```
Created --> Waiting --> Active --> Ended
              |           |
              v           v
           Locked     Paused
```

- **Created**: Room exists but no participants yet
- **Waiting**: Waiting room enabled, participants queue for host approval
- **Active**: Meeting in progress, media flowing
- **Paused**: Host paused the meeting (all media muted)
- **Locked**: No new participants allowed
- **Ended**: Meeting over, resources released

### Room Configuration

```rust
struct RoomConfig {
    max_participants: usize,       // default: 100
    enable_waiting_room: bool,     // default: false
    enable_recording: bool,        // default: false
    enable_chat: bool,             // default: true
    enable_screen_share: bool,     // default: true
    enable_hand_raise: bool,       // default: true
    auto_mute_on_join: bool,       // default: false for small rooms
    password: Option<String>,      // room password
    allowed_codecs: Vec<Codec>,    // default: [Opus, VP9, H264]
    max_video_bitrate: u32,        // default: 2_500_000 (2.5 Mbps)
    max_audio_bitrate: u32,        // default: 128_000 (128 kbps)
}
```

### Participant Roles

| Role | Permissions |
|------|------------|
| Host | All permissions, end room, manage waiting room |
| Moderator | Mute others, kick participants, manage screen share |
| Presenter | Screen share, unmute self |
| Participant | Unmute self, chat, raise hand |
| Viewer | View-only (webinar mode) |

### Recording (planned)

Server-side recording via compositor:
1. SFU captures forwarded tracks
2. Compositor renders grid layout (ffmpeg or custom)
3. Output written to S3/MinIO as MP4
4. Individual track recording also available (separate files per participant)

Recording control: start/stop via REST API, MCP, or meeting controls.

## GPU TUI Client

The client is a standalone GPU application for joining meetings from the terminal.
Not browser-based -- uses the pleme-io GPU rendering stack.

### Client Layout

```
+-------+-------+-------+
| User A| User B| User C|
| (vid) | (vid) | (vid) |
+-------+-------+-------+
| User D| User E| Screen|
| (vid) | (vid) | Share |
+-------+-------+-------+
| [Mic] [Cam] [Share] [Chat] [Hand] [Leave] |
+--------------------------------------------+
| Chat: alice: hello everyone                |
|        bob: hi!                            |
| [type message...]                          |
+--------------------------------------------+
```

Adaptive grid layout: 1x1 for solo, 1x2 for two, 2x2 for four, NxM for more.
Active speaker highlighted. Screen share takes priority (large tile).

### Client Dependencies

| Library | Used For |
|---------|----------|
| **garasu** | GPU context, video frame rendering, text |
| **madori** | App framework (event loop, render callback) |
| **egaku** | Controls (buttons, chat input, participant list) |
| **irodzuki** | Theming (meeting UI colors) |
| **shikumi** | Client config (`~/.config/taimen/taimen.yaml`) |

### WebRTC in Rust

Client-side WebRTC via `webrtc-rs` (pure Rust WebRTC implementation):
- ICE agent for NAT traversal
- DTLS for encrypted media transport
- SRTP for audio/video
- Opus decoding (audio)
- VP9/H.264 decoding (video) -- decoded frames uploaded as garasu textures

## Configuration (shikumi)

### Server Config

File: `~/.config/taimen/taimen.yaml` (dev) or `/etc/taimen/taimen.yaml` (prod)
Env override: `$TAIMEN_CONFIG`
Env prefix: `TAIMEN_`

```yaml
server:
  listen: "0.0.0.0:8443"
  public_url: "https://meet.example.com"
  tls:
    cert: "/etc/taimen/cert.pem"
    key: "/etc/taimen/key.pem"

stun:
  - "stun:stun.l.google.com:19302"
  - "stun:stun1.l.google.com:19302"

turn:
  - url: "turn:turn.example.com:3478"
    username: "taimen"
    credential_command: "cat /run/secrets/turn-credential"

rooms:
  max_participants: 100
  default_video_bitrate: 2500000
  default_audio_bitrate: 128000
  idle_timeout: 3600       # close empty rooms after 1 hour
  max_duration: 28800      # 8 hour max meeting duration

recording:
  enabled: false
  storage: s3
  s3_bucket: "taimen-recordings"
  s3_endpoint: "http://minio:9000"

auth:
  jwt_secret_command: "cat /run/secrets/jwt-secret"
  allow_anonymous: true    # allow joining without auth
  room_passwords: true     # allow password-protected rooms
```

### Client Config

File: `~/.config/taimen/client.yaml`

```yaml
display_name: "User"
default_audio_device: "default"
default_video_device: "default"
video_resolution: "720p"
theme:
  name: nord
keybindings: {}
```

## MCP Server (kaname)

Stdio transport for AI meeting management.

| Tool | Parameters | Description |
|------|-----------|-------------|
| `create_room` | [config] | Create a new meeting room |
| `join_room` | room_id, [display_name] | Join a room |
| `leave_room` | room_id | Leave a room |
| `list_rooms` | | List active rooms |
| `list_participants` | room_id | List room participants |
| `mute_participant` | room_id, participant_id, kind | Mute audio/video |
| `kick_participant` | room_id, participant_id, [reason] | Remove from room |
| `start_recording` | room_id | Start recording |
| `stop_recording` | room_id | Stop recording |
| `get_room_stats` | room_id | Room statistics (duration, participants, bandwidth) |
| `set_room_config` | room_id, key, value | Update room config |
| `end_room` | room_id | End meeting for everyone |
| `status` | | Server health status |

### AI Meeting Assistant Use Case

The MCP server enables AI-powered meeting automation:
- **Transcription bot**: Join room via MCP, capture audio, transcribe in real-time
- **Summary bot**: Generate meeting summaries from transcript
- **Action item tracker**: Extract action items from conversation
- **Translation bot**: Real-time translation overlay

## Rhai Scripting (soushi)

Scripts in `~/.config/taimen/scripts/*.rhai`. Server-side meeting automation.

### API

```rhai
// Room management
taimen.create_room(config)           // Create room
taimen.end_room(room_id)             // End room

// Participant management
taimen.participants(room_id)         // List participants
taimen.mute(participant_id, "audio") // Mute participant
taimen.kick(participant_id, reason)  // Kick participant

// Meeting flow
taimen.lock_room(room_id)            // Prevent new joins
taimen.unlock_room(room_id)          // Allow new joins

// Event hooks
fn on_join(room_id, participant) { ... }
fn on_leave(room_id, participant_id) { ... }
fn on_hand_raise(room_id, participant_id) { ... }
fn on_room_start(room_id) { ... }
fn on_room_end(room_id) { ... }
```

### Example: Auto-Record and Notify

```rhai
fn on_room_start(room_id) {
    taimen.start_recording(room_id);
    log("Recording started for room " + room_id);
}

fn on_room_end(room_id) {
    taimen.stop_recording(room_id);
    log("Recording saved for room " + room_id);
}

fn on_join(room_id, participant) {
    if taimen.participants(room_id).len() > 10 {
        taimen.mute(participant.id, "audio");
        log("Auto-muted " + participant.name + " (large meeting)");
    }
}
```

## Nix Integration

### flake.nix (planned)

```
packages.${system}.server     -- taimen server binary
packages.${system}.client     -- taimen GPU TUI client binary
packages.${system}.container  -- OCI container image (server only)
packages.${system}.default    -- server
overlays.default              -- pkgs.taimen, pkgs.taimen-client
nixosModules.default          -- services.taimen (NixOS service)
homeManagerModules.default    -- blackmatter.components.taimen (client config)
devShells.${system}.default   -- dev environment
```

### NixOS Module (planned)

`services.taimen`:
- `enable` -- enable taimen systemd service
- `package` -- taimen server package
- `settings` -- attrs -> `/etc/taimen/taimen.yaml`
- `openFirewall` -- open listen port in firewall
- `coturn.enable` -- provision local TURN server

### K8s Deployment (via k8s repo)

Deployed as a Deployment + Service:
- FluxCD reconciliation
- TURN server as sidecar or separate deployment
- MinIO for recording storage
- Ingress via Traefik with TLS + WebSocket upgrade

## Implementation Roadmap

### Phase 1 -- Data Model (current)
- [x] `Room` with `RoomConfig` and `RoomState` state machine
- [x] `Participant` with `ParticipantRole` and state (muted, hand raised, etc.)
- [x] `SignalMessage` enum (14 variants: Join, Leave, Offer, Answer, IceCandidate, Mute, Unmute, RaiseHand, LowerHand, ScreenShare, StopScreenShare, ChatMessage, Kick, EndRoom)
- [x] `MediaTrack` and `MediaConfig` types
- [x] `TaimenConfig` with shikumi config
- [x] `TaimenError` error types
- [x] Serde roundtrip tests for all signal messages

### Phase 2 -- Signaling Server
- [ ] axum HTTP server with WebSocket upgrade at `/ws/{room_id}`
- [ ] denshin connection manager (per-room connection tracking)
- [ ] SignalMessage dispatch (relay Offer/Answer/IceCandidate between peers)
- [ ] Room lifecycle (create on first join, destroy on last leave)
- [ ] Heartbeat mechanism (detect disconnected participants)
- [ ] REST API: `POST /rooms`, `GET /rooms`, `GET /rooms/:id`

### Phase 3 -- SFU (eizou)
- [ ] WebRTC media reception (DTLS + SRTP)
- [ ] Track router: publisher -> subscriber forwarding
- [ ] Simulcast layer selection
- [ ] Bandwidth estimation (REMB or TWCC)
- [ ] Audio level detection (for active speaker)
- [ ] Opus audio forwarding
- [ ] VP9/H.264 video forwarding

### Phase 4 -- Auth & Rooms (kenshou)
- [ ] JWT-based room tokens
- [ ] Room passwords
- [ ] Waiting room (host approval queue)
- [ ] Role-based permissions (host, moderator, presenter, participant, viewer)
- [ ] Room locking (prevent new joins)
- [ ] Rate limiting

### Phase 5 -- GPU TUI Client
- [ ] madori app scaffold (window, event loop, render callback)
- [ ] WebRTC peer connection via webrtc-rs
- [ ] Video frame decoding -> garasu texture upload
- [ ] Adaptive grid layout for video tiles
- [ ] Meeting controls (egaku buttons: mute, camera, share, chat, hand, leave)
- [ ] In-meeting chat panel
- [ ] Active speaker detection + highlight
- [ ] Screen share viewing

### Phase 6 -- Recording
- [ ] Server-side track capture
- [ ] Composite recording (grid layout, ffmpeg)
- [ ] Individual track recording
- [ ] S3/MinIO upload
- [ ] Recording metadata (start/stop times, participants)
- [ ] REST API + MCP control

### Phase 7 -- MCP & Scripting
- [ ] kaname MCP admin server
- [ ] soushi Rhai scripting engine
- [ ] Meeting flow automation hooks
- [ ] AI meeting assistant integration

### Phase 8 -- Advanced Features
- [ ] Screen sharing from GPU TUI client
- [ ] Virtual backgrounds (GPU shader-based, garasu pipeline)
- [ ] Breakout rooms (sub-rooms with return-to-main)
- [ ] Reactions/emoji overlay
- [ ] End-to-end encryption (Insertable Streams)
- [ ] Dial-in (SIP gateway, future)

## Design Decisions

### Why WebRTC SFU (not custom media protocol)?
WebRTC is the industry standard for real-time media. Browser clients work
out of the box. The SFU architecture (via eizou) provides low-latency
forwarding without transcoding. Custom protocols would require custom
clients and lose browser compatibility.

### Why axum + denshin (not dedicated signaling server)?
The signaling server is lightweight -- it only relays SDP offers/answers and
ICE candidates. Axum handles HTTP (REST API) and WebSocket (signaling) in
one binary. denshin provides connection management and room-based event
routing on top of axum's WebSocket support.

### Why both server and client in one repo?
The signaling protocol (`SignalMessage`) is shared between server and client.
Keeping them in one repo ensures protocol compatibility. The client is an
optional build target (`cargo run -- client`), not required for server-only
deployment.

### Why webrtc-rs for client (not browser)?
The GPU TUI client runs in a terminal, not a browser. `webrtc-rs` is a pure
Rust WebRTC implementation that provides ICE, DTLS, and SRTP without browser
dependencies. Decoded video frames are uploaded as GPU textures for rendering
via garasu.

### Why Opus (not other codecs)?
Opus is the mandatory WebRTC audio codec, provides excellent quality at
low bitrates (64-128 kbps), handles both voice and music, and has low
latency (5-66ms). All WebRTC implementations support it.

### Why VP9 preferred (not AV1)?
VP9 has broader hardware decoder support (most GPUs from 2015+), while AV1
hardware decoding is still limited to recent GPUs (2020+). VP9 provides
good compression for video conferencing. AV1 support can be added later
as hardware adoption grows.

### Why shikumi for server config?
Consistency with the pleme-io ecosystem. Even though servers typically use
static config files, shikumi's hot-reload enables runtime config changes
(e.g., adjusting rate limits, enabling recording) without server restart.
The `password_command` pattern integrates with secret management.
