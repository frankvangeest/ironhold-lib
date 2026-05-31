# Feature: Multiplayer Networking (Three Forms)

_Status: Draft_
_Planned at: `9f1b459` (2026-05-31)_

---

> ## Pre-implementation checklist
>
> The following must be resolved **before any networking code is written**. Each item is a blocker or a decision that changes the implementation significantly.
>
> - [ ] **Beta 0.5 (Deterministic Tick + Replay) must be complete.** Networking sync requires a fixed-tick simulation loop and deterministic RNG. Without this foundation, any multiplayer implementation will produce desync that is impossible to diagnose. Do not start networking until the replay demo works.
>
> - [ ] **Library spike: confirm Bevy Lightyear fits the action-queue architecture.** Lightyear is the strongest current candidate (native + WASM, rollback/prediction built-in, WebTransport/WebRTC). Before committing, run a one-day spike: wire Lightyear's input replication to `InputActionMessage` and verify that `ActionQueue` actions can be dispatched server-side from replicated inputs. If Lightyear is a poor fit, evaluate `bevy_renet` (UDP/WebSocket, no built-in rollback) and `matchbox_socket` (WebRTC peer-to-peer, WASM-friendly). Document the decision in this file before proceeding.
>
> - [ ] **WASM transport confirmed.** Web clients cannot open raw TCP/UDP sockets. The chosen library must support WebSocket or WebRTC/WebTransport for WASM targets. Confirm this compiles and connects in a browser before writing game-sync code.
>
> - [ ] **`ironhold_core` must stay platform-agnostic.** All server-side simulation code goes in a new `ironhold_server` crate (Form 3 only). `ironhold_core` may gain networking traits/types but must never contain server-only or client-only platform assumptions. Review any new code in `ironhold_core` against this rule.
>
> - [ ] **Relay / signaling service decision (Form 2).** For internet play, a connection broker is needed for NAT traversal. Two options: (a) self-hosted lightweight relay (Matchbox server, ~100 lines of Go/Rust); (b) managed service (Agones, Unity Relay equivalent). Decide before designing Form 2 — it affects the project's operational dependencies.

---

## What

Ironhold needs to support multiplayer in three distinct forms, each with different complexity and infrastructure requirements. This feature file covers all three so the design decisions are made consistently, but they are implemented as separate milestones.

**Form 1 — LAN / local co-op (listen server, same network)**
One player hosts in the native runner; others join on the same local network. No external services required. The hosting player runs both the server simulation and a client renderer.

**Form 2 — Internet player-hosted (listen server + relay)**
Same as Form 1 but playable over the internet. Requires a lightweight signaling / relay service for NAT traversal. Web (WASM) clients must be able to join, which constrains the transport layer to WebSocket or WebRTC.

**Form 3 — Dedicated authoritative server**
A headless server binary (`ironhold_server` crate) runs the full simulation. Clients are thin renderers that send inputs and receive state. The server loads and ticks `ironhold_core` with no Bevy render plugins. Most scalable, most complex — suited for competitive games.

---

## Why

The current Beta 0.6 milestone describes "server-authoritative networking" as a single item, but the three forms have different prerequisites, different infrastructure, and different use cases. Splitting them:

- Lets Form 1 (LAN co-op) ship much earlier, proving the sync architecture without internet complexity.
- Gives Form 2 its own design space — the relay/signaling service is an operational dependency that needs an explicit decision.
- Defers Form 3 (dedicated server + new crate) until there is a clear demand for it.

---

## Form 1 — LAN / Local Co-op

### Approach

- One `ironhold_native` instance runs in host mode (`--host` flag or a `HostGame` action).
- Other players launch with `--join <host-ip>` or a `JoinGame(address)` action.
- The host ticks the fixed simulation (Beta 0.5 tick loop) and replicates state to clients.
- Clients send `InputActionMessage` packets to the host; the host authorises and simulates.
- Initial sync strategy: **lock-step** (simpler to implement, acceptable on LAN latency). Rollback/prediction can be added in Form 2 if needed.

### New RON actions / events

```ron
// Actions
HostGame(port: 7777)
JoinGame(address: "192.168.1.10:7777")
DisconnectGame
KickPlayer(player_id: "p2")

// Events into the pipeline
multiplayer.host_started
multiplayer.player_joined:{player_id}
multiplayer.player_left:{player_id}
multiplayer.disconnected
```

### New crate additions

None — Form 1 lives entirely in `ironhold_native` (host/join flag) and `ironhold_core` (sync protocol, replicated components). No `ironhold_server` crate needed.

### Tasks

- [ ] Library spike (see pre-implementation checklist)
- [ ] Fixed-tick input capture in `ironhold_core` (depends on Beta 0.5)
- [ ] `HostGame` / `JoinGame` / `DisconnectGame` action variants in `schema/actions.rs`
- [ ] `NetworkSession` resource (host vs. client mode, connected player list)
- [ ] Input replication: client sends `InputActionMessage` each tick; host applies all inputs before ticking
- [ ] State replication: define which components are replicated (position, animation state, stats, variables)
- [ ] `multiplayer.*` events into the pipeline
- [ ] `--host` / `--join` CLI flags in `ironhold_native`
- [ ] LAN co-op demo scene (2-player, split input bindings)
- [ ] Integration tests: two instances connect, inputs replicated, state converges
- [ ] Docs

### Open questions

- Lock-step vs. client prediction for Form 1? Lock-step is simpler but any packet loss freezes both clients. Client prediction can be added later if LAN latency is insufficient.
- Which components are replicated? Minimum viable set: `Transform`, `AnimationState`, `StatMap`, `GameVariables`. Define a `#[replicated]` marker or use Lightyear's replication rules.
- Player identity: string `player_id` (session-scoped) or persistent UUID? Session-scoped is fine for Form 1.

---

## Form 2 — Internet Player-Hosted (Listen Server + Relay)

### Approach

Extends Form 1. The host still runs the simulation, but players connect over the internet. Requires:

1. A **signaling / relay service** so clients can discover and reach the host through NAT.
2. A WASM-compatible transport (WebSocket or WebRTC/WebTransport) so browser players can join.

The relay service is responsible only for handshaking — it does not simulate game logic. Once a connection is established, traffic flows directly between host and client (peer-to-peer if NAT allows, through relay if not).

### New RON actions / events

```ron
// Additional actions beyond Form 1
CreateLobby(name: "my game", max_players: 4)
JoinLobby(lobby_id: "abc123")
ListLobbies   // emits multiplayer.lobbies_listed:[...] into pipeline

// Additional events
multiplayer.lobby_created:{lobby_id}
multiplayer.lobby_joined:{lobby_id}
multiplayer.lobby_listed:{json_payload}
```

### Infrastructure

Two options — **decision required before Form 2 starts**:

| Option | Description | Operational cost |
|---|---|---|
| Self-hosted Matchbox server | Open-source WebRTC signaling server, ~1 container | Low — deploy once, no per-connection cost |
| Managed relay (e.g. Agones / custom) | Managed infrastructure | Higher — ongoing cost, external dependency |

Recommendation: self-hosted Matchbox is the simplest path for a small engine project. The `matchbox_socket` crate integrates with Bevy and is WASM-compatible.

### Tasks

- [ ] Relay service decision and deployment
- [ ] WASM transport confirmed in browser (see pre-implementation checklist)
- [ ] Lobby system: `CreateLobby` / `JoinLobby` / `ListLobbies` actions
- [ ] `LobbyList` UI component (scene RON) for in-game lobby browser
- [ ] Connection handshake through relay → direct peer-to-peer fallback through relay
- [ ] Latency/jitter tolerance: add client prediction or increase input buffer size for internet latency
- [ ] Internet co-op demo scene (accessible from WASM build)
- [ ] Docs: relay setup guide for project deployers

### Open questions

- Should lobby state (player list, game name) be managed by the relay service or by the host broadcasting through the game protocol? Simpler to use the relay for lobby metadata.
- Max player count: enforce in RON (`max_players` on `CreateLobby`) or in the host simulation?
- Does Form 2 need rollback netcode, or is the input buffer approach (Form 1) sufficient for internet latency? This depends on the target game genre — action games need rollback, turn-based or slow-paced games can tolerate higher buffer sizes.

---

## Form 3 — Dedicated Authoritative Server

### Approach

A new `ironhold_server` crate — headless Bevy with no render or window plugins. The server:

- Loads `ironhold_core` in server mode (no `WgpuPlugin`, no `WindowPlugin`)
- Ticks the fixed simulation loop
- Receives `InputActionMessage` packets from all clients
- Runs `action_executor_system` and all gameplay systems
- Replicates authoritative state to clients each tick
- Clients run `ironhold_web` or `ironhold_native` as thin renderers (no simulation, only interpolation + prediction)

### New crate: `ironhold_server`

```
crates/
  ironhold_core/       ← unchanged; gains server-mode feature flag if needed
  ironhold_native/     ← desktop client/host runner (unchanged)
  ironhold_web/        ← WASM client runner (unchanged)
  ironhold_server/     ← NEW: headless simulation server
```

`ironhold_server/src/main.rs` calls a new `start_server(project_path, port)` entry point in `ironhold_core` that omits all render plugins. The server binary parses `--project` and `--port` flags.

### Tasks

- [ ] Feature file for `ironhold_server` crate (separate, more detailed spec)
- [ ] `start_server()` entry point in `ironhold_core` (no render plugins, no window)
- [ ] `ironhold_server` crate scaffolding (Cargo.toml, main.rs)
- [ ] Client-mode `ironhold_native` / `ironhold_web`: receive replicated state, suppress local simulation
- [ ] Anti-cheat surface: server is authoritative — clients cannot inject actions directly; all inputs validated server-side
- [ ] Server admin actions via CLI or RON config: `kick`, `ban`, `change_scene`
- [ ] Dedicated server demo (deploy script + multiplayer demo scene)
- [ ] Docs: server deployment guide

### Open questions

- Does `ironhold_core` need a `server` feature flag to compile without render/window deps, or is it cleaner to keep all server-only code in `ironhold_server`? Feature flag approach avoids a conditional dependency tree; separate crate is cleaner but duplicates some wiring.
- Cheat prevention: for a game engine targeting indie developers, how much server-side validation is in scope for v1? Minimum viable: inputs are rate-limited and clamped to valid ranges; full anti-cheat is out of scope.
- Deployment: should the engine provide a `Dockerfile` for `ironhold_server`? Useful for Form 3 but adds maintenance surface.

---

## Milestone split (recommended)

| Milestone | Form | Gate |
|---|---|---|
| Beta 0.5 | Deterministic tick + replay | Must ship first |
| Beta 0.6 | Form 1: LAN co-op | After 0.5 |
| Beta 0.7 | Loading & Preloading (existing) | Can run in parallel with Form 1 |
| Beta 0.8 | Form 2: Internet player-hosted + relay | After 0.6 |
| Beta 0.9+ | Form 3: Dedicated server | After 0.8, separate feature file |

---

## Acceptance criteria (per form)

**Form 1**
- Given two `ironhold_native` instances on the same LAN, one hosting and one joining, player positions and animations are synchronised within one fixed-tick frame.
- Given a player disconnecting mid-session, the `multiplayer.player_left:{id}` event fires and the session continues with remaining players.
- Given a `HostGame` action in `rules.ron`, the engine starts listening for connections — no code change required.

**Form 2**
- Given a host creating a lobby and a WASM client opening the lobby browser, the WASM client can see and join the lobby over the internet.
- Given 100 ms of simulated packet latency, the game remains playable (no visible freeze or desync).
- Given a relay service outage after a connection is established, the session degrades gracefully (direct connection continues if NAT allows).

**Form 3**
- Given a running `ironhold_server` binary, two WASM clients can connect and play simultaneously with the server as authority.
- Given a client attempting to inject an invalid action directly, the server rejects it and the client state is corrected on the next tick.
- Given the server loading a project via `--project particles_demo`, all RON rules execute server-side identically to the native single-player run.
