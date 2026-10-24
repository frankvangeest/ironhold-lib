# Feature: Nakama backend spike (throwaway)

_Status: In Progress (started 2026-10-24; S1 done, S2 and E1/E2 code written, waiting on the server key file; see "Progress log")_
_Planned at: `98b23b7` (2026-10-23)_

Decision being informed: `planning/investigations/nakama_backend_viability.md` (recommendation "both, layered";
this spike is its section 8, turned into a plan with measurements and go/no-go criteria).

## What
A short, throwaway experiment: a standalone Rust test program, built for **native and wasm32**, that talks to a Nakama
server and produces numbers. It answers whether a browser-hosted Ironhold build could use Nakama for sign-in, cloud
storage, a leaderboard, lobby discovery and a relayed match, and what that costs in latency, bandwidth and client code.
No engine change, no RON surface, no new `ironhold_core` code.

## Why
The investigation could not verify the things that decide the architecture: whether a hand-written Rust client for
Nakama's REST API and realtime socket is small, whether it works from a browser tab on an HTTPS origin, and whether a
WebSocket relay is fast enough for our tempo. Opinions are cheap; a 1-2 day measurement is cheaper than a wrong
commitment in Beta 0.6/0.8.

## Scope boundary (important)
- **In this repo:** the spike client, the measurement scripts and the written result.
- **Not in this repo:** hosting. Running Nakama (compose, Postgres, TLS, Cloudflare Tunnel hostname, DNS, backups,
  secrets) is tracked in the homelab repo: `C:\workspace\frank\homelab\planning\features\nakama-deployment.md`. This spike
  consumes a reachable Nakama endpoint and records which endpoints and versions it used; it does not configure them.
- **Out of scope:** engine integration, RON Actions, state replication, Lightyear/replicon/matchbox compatibility with our
  Bevy version, authoritative match handlers, the TypeScript runtime module, Hiro/Satori, pricing.

## Where the code lives
A standalone crate at `spikes/nakama_spike/`, **outside** the Cargo workspace (its own `Cargo.toml` with an empty
`[workspace]` table), so it cannot change `Cargo.lock`, feature unification or build times of the engine crates. It uses the
shared `CARGO_TARGET_DIR`. It is not shipped, not documented in `docs/`, and is deleted (or promoted into a real crate under a
new plan) when the decision is made.

## Prerequisites
1. A reachable Nakama endpoint from the homelab plan, in two stages (decided 2026-10-23: the homelab starts **LAN-only**):
   - **Stage 1 (LAN-only, plain `http://` / `ws://` on the LAN):** enough for E1-E6 natively and in a browser page served
     from `http://localhost` (a non-HTTPS page may open plain `ws://`). The spike starts here.
   - **Stage 2 (public hostname with a real certificate):** needed only for the HTTPS-origin runs: the `wss://` and CORS
     check, E7 (idle socket through the tunnel), and the "via the public hostname" latency column. Those runs wait for the
     homelab's stage 2; they do not block the rest.
2. Until the homelab stack is up, the native parts can run against a throwaway local container from the vendor's
   documented Docker Compose (Nakama + Postgres) on this machine, if Docker is available. Do not put that compose file in
   this repo.
3. Record the exact Nakama server version, since the client is written against it.

## Experiments

Everything below is measured **native vs browser** and, once the homelab's stage 2 exists, **LAN vs via the public hostname**.

| ID | Experiment | What we learn |
|---|---|---|
| E1 | REST: authenticate with a device id (create + re-login), store the session token and refresh it | Effort of the REST client; token handling in wasm |
| E2 | REST: write and read a storage object (cloud-save shape: a few KB of JSON), including a version conflict | Storage semantics and error codes |
| E3 | REST: submit a leaderboard score and list the top 10. **Needs a server runtime module** (see Progress log): a client cannot create a leaderboard | Leaderboard shape; the real cost of "meta features need server code" |
| E4 | Realtime socket: connect, join/create a **relayed** match (with a label for lobby listing), list matches, leave | Effort of the socket protocol; lobby discovery fit for `ListLobbies` |
| E5 | Two clients (one native, one browser tab) exchange match data; measure round-trip and one-way delay, loss, ordering | The number that decides whether a WebSocket relay is viable |
| E6 | Matchmaker: two clients add a ticket with properties and get matched | Whether `FindMatch` can be built on it |
| E7 | Idle socket for 5+ minutes through the public hostname, then reconnect behaviour | Whether the tunnel/proxy cuts quiet sockets; reconnect effort |

### Measurement protocol (E5)
- Payload sizes: 64 B, 512 B and 1400 B (stay under the documented ~1500-byte message guidance).
- Rates: 10, 20 and 30 messages per second per client, 60 seconds each, 2 clients, then 4.
- A representative snapshot is ~400 B (four players of position, rotation and a few stats); use that as the main case.
- Record per run: p50 / p95 / p99 round-trip and one-way delay (clock offset handled by echo round trips, not wall-clock
  comparison), message loss, reordering, server and client bandwidth, and any server-side disconnect log lines.
- Compare JSON-envelope and protobuf-envelope sockets once, to see the real size and CPU difference (base64 of match data
  in the JSON format adds about a third).

## Tasks
- [ ] S1 Prerequisites: endpoint reachable, server version recorded, device-id sign-in works with `curl`
- [ ] S2 Scaffold `spikes/nakama_spike/` (outside the workspace), native + wasm32 targets, minimal HTML harness served from the
      existing dev server for the browser runs (localhost page: plain `ws://` allowed; HTTPS page for the `wss://` runs)
- [ ] S3 E1-E3 (REST) in native, then wasm; choose an HTTP client that works on both targets
- [ ] S4 E4: realtime socket envelope (JSON first); pick a WebSocket crate that works on both targets; create/join/list/leave
- [ ] S5 E5 measurement harness with the protocol above; keep raw numbers in a CSV next to the report
- [ ] S6 E6 matchmaker; E7 idle and reconnect (E7 and the HTTPS/`wss` runs wait for the homelab stage 2)
- [ ] S7 Repeat E5 with the protobuf envelope (generate types from the vendor's published `realtime.proto`)
- [ ] S8 Write the result: append "Spike result" to `planning/investigations/nakama_backend_viability.md`, state go/no-go
      against the criteria below, estimate the effort of a maintained client crate, update the backlog items, then delete or
      archive `spikes/nakama_spike/`

## Progress log

**2026-10-24**
- S1 done: endpoint `http://192.168.178.106:7350` reachable, Nakama **3.41.0**, device-id sign-in works (checked with `curl` on the
  LXC itself, tokens redacted); wrong or default server key is rejected with HTTP 401.
- S2 done (native): `spikes/nakama_spike/` standalone crate (`api.rs` protocol, `config.rs`, `main.rs` native driver). Probe from this
  machine over the LAN: `GET /` 200, sign-in with the default key 401 in single-digit milliseconds after the first (cold) request.
- E1 and E2 are written (`e1`, `e2`, `all` subcommands) and need the server key, which the spike reads from `NAKAMA_SERVER_KEY` or a
  local file outside the repo (`~/.nakama_spike_server_key`, never committed, never printed).
- **Finding (E3 blocker):** Nakama's own docs say a leaderboard "can be created via server-side code at startup or within a
  registered function" (Go/TypeScript/Lua); clients can only submit to and list existing ones. So E3 needs a small server runtime
  module (JavaScript, loaded from `/nakama/data/modules` in the container). That means a homelab change (bind-mount a modules
  directory; tracked in `homelab/planning/features/nakama-deployment.md`) and is direct evidence for the investigation's point that
  even basic meta features need server-side code unless we ship one generic module.

## Go / no-go criteria
**Go** (write the `ironhold_backend` abstraction plan with a Nakama adapter) when all hold:
- A browser tab on an HTTPS origin connects over `wss://` and completes E1-E4 with no CORS or mixed-content workaround.
- E5 at 20-30 Hz with a ~400 B snapshot for 2-4 players: p95 round trip within a budget we will write down *before* running
  (proposal: under 150 ms on the LAN path and under 250 ms via the public hostname), no sustained loss, no server disconnects.
- The client for E1-E6 is about 1000 lines or fewer and has no unsafe code or heavyweight dependencies on wasm.
- No licence, database or operations surprise on the server side.

**Change course** when:
- The relay is too slow for our tempo: keep Nakama for meta services (E1-E3, E6) and use Matchbox/WebRTC for gameplay transport.
- The realtime protocol is impractical from Rust/wasm: use REST-only Nakama features (sign-in, storage, leaderboard) and
  a separate small lobby service.
- Operating Nakama on the homelab is too fragile or exposed: consider Heroic Cloud, or a small own service for lobbies only.

## Risks
- A single homelab node and a residential connection set the latency floor, not Nakama; compare against a second vantage point
  (a phone on mobile data) before blaming either.
- Cloudflare Tunnel may behave differently for long-lived WebSockets than the LAN path; E7 exists for this.
- The Nakama API can change between minor versions; pin the version and record it with the numbers.
- Spike code quality is irrelevant but its **numbers** must be reproducible: keep the exact commands and server version in the report.

## Open questions
- JSON or protobuf envelope as the primary format? (Plan: JSON to get working, protobuf for the final numbers.)
- Which two or three HTTP / WebSocket crates to try first? (Decide at S3/S4 from what compiles for wasm32; the spike crate is outside the Bevy workspace, so no engine version constrains it.)
- Who owns the Nakama console credentials and server key for the run? (Homelab plan; the spike only receives an endpoint and a
  server key at runtime, never in this repo.)

## Acceptance criteria
- Given the spike is complete, when the report is read, then it states the Nakama version, the endpoints used (host names
  only), every E1-E7 outcome, the E5 numbers with the commands that produced them, and an explicit go / change-course decision.
- Given the decision is "go", when the report is closed, then a follow-up plan exists for the backend abstraction and the
  client crate, with an effort estimate based on the spike's code size.
- Given the spike is closed, when the repo is inspected, then `spikes/nakama_spike/` is gone or archived, no secret or server
  key is committed, and no engine crate has changed.
