# Feature: Nakama backend spike (throwaway)

_Status: In Progress (started 2026-10-24; plan revised after a system-architect plan review the same day; see "Progress log")_
_Planned at: `98b23b7` (2026-10-23); revised at `a6b0170` (2026-10-24)_

Decision being informed: `planning/investigations/nakama_backend_viability.md` (recommendation "both, layered";
this spike is its section 8, turned into a plan with measurements and go/no-go criteria).

## What
A throwaway experiment: a standalone Rust program, built for **native and wasm32**, that talks to a Nakama server and produces
numbers. It answers whether a browser-hosted Ironhold build could use Nakama for sign-in, cloud storage, a leaderboard, lobby
discovery and a relayed match, and what that costs in latency, bandwidth, server-side code and client code. No engine change, no
RON surface, no new `ironhold_core` code.

## Why
The investigation could not verify the things that decide the architecture: whether a hand-written Rust client for Nakama's
REST API and realtime socket is small, whether it works from a browser tab, and whether a WebSocket relay is fast enough for our
tempo. Opinions are cheap; a few days of measurement is cheaper than a wrong commitment in Beta 0.6/0.8.

## Scope boundary (important)
- **In this repo:** the spike client, one tiny server-side module for the leaderboard, the measurement scripts, and the written result.
- **Not in this repo:** hosting. Running Nakama (compose, Postgres, TLS, Cloudflare Tunnel hostname, DNS, backups, the real
  secrets) is tracked in the homelab repo: `C:\workspace\frank\homelab\planning\features\nakama-deployment.md`. This spike consumes a
  reachable endpoint and records the server version and `socket.*` settings it ran against. The one thing it needs from the
  homelab beyond that is a bind-mounted **modules directory** for E3 (a homelab task; the module source lives here).
- **Out of scope:** engine integration, RON Actions, state replication, Lightyear/replicon/matchbox compatibility with our Bevy
  version, authoritative match handlers, a general-purpose runtime module, Hiro/Satori, pricing.

## Facts the plan is built on (checked 2026-10-24 against the vendor docs and the crate sources)
- Nakama defaults: `session.token_expiry_sec` **60**, `refresh_token_expiry_sec` **3600**, `socket.max_message_size_bytes` **4096**
  (incoming, per message), `socket.outgoing_queue_size` **16** (a client that reads too slowly is disconnected),
  `ping_period_ms` 15000, `pong_wait_ms` 25000, `matchmaker.interval_sec` **15**.
- Leaderboards can only be created by server-side code (verbatim from the docs); clients can submit to and list existing ones.
  A score submitted by a client is trusted unless the leaderboard is authoritative.
- A **relayed** match takes only an optional `name` (a stable match id); the docs describe no label for relayed matches.
- `ewebsock` on native runs one loop: send at most one queued message, then read with a 10 ms `read_timeout`, and never sets
  `TCP_NODELAY`. On wasm it is a thin wrapper over the browser `WebSocket`.
- Nakama's JSON envelope encodes `int64` fields (`op_code`, scores) as strings and match data as base64.

## Where the code lives
A standalone crate at `spikes/nakama_spike/`, **outside** the Cargo workspace (its own `[workspace]` table, required because the
root workspace lists its members explicitly), using the shared `CARGO_TARGET_DIR` and its own committed `Cargo.lock`. Not shipped,
not documented in `docs/`, deleted (or promoted under a new plan) when the decision is made.

Structure (decided by the review):
- **Protocol layer**: pure message types and parsing, no networking (REST builders/parsers; socket envelope as serde types; protobuf
  via `prost` later). This is the part a real client crate would reuse and a later Bevy integration would drive through channels.
  **The "code size" criterion counts only this layer**, not the harness, generated code or the HTML page.
- **Transport layer**: `ehttp` for REST (native + wasm). WebSocket: **`ewebsock` for wasm only**; on native, **`tokio-tungstenite`
  with `set_nodelay(true)` and a split reader/writer**. `ewebsock` on native is excluded from every timing run because its 10 ms
  send/read loop and missing NODELAY would be measured as "Nakama is slow". Native `wss://` (needs a TLS feature that compiles C
  code under MSVC) is left off in stage 1.
- `spikes/nakama_spike/.gitignore`: `pkg/`, `.env`, `*.local`. wasm output must never be committed.

## Secrets and tokens
- The **server key is not a real secret**: Nakama embeds it in every client by design (its own browser client ships it). It is still
  kept out of the repo as hygiene, supplied at runtime by `NAKAMA_SERVER_KEY` or a local file outside the repo (never printed).
  The real secrets (console credentials, runtime HTTP key, session and refresh encryption keys) stay entirely in the homelab repo.
- **Session tokens are the leaky item**: the WebSocket URL carries `?token=...` (browsers cannot set headers on a WebSocket). Never
  log full socket URLs or `Authorization` headers; keep tokens out of CSVs and the report. Stage 2 proxies and Cloudflare may log it.
- The report and the code contain host names only, **no LAN IP addresses** (the endpoint is `NAKAMA_URL`; the crate defaults to `localhost`).

## Prerequisites
1. A reachable Nakama endpoint (homelab plan), two stages: **stage 1** LAN-only plain `http://` / `ws://`; **stage 2** public
   hostname with a real certificate. Stage 1 is enough for E1-E6 natively and in a browser page served from `http://localhost`.
   **Correction:** a page on `http://localhost` calling `http://<lan-host>:7350` is already cross-origin, so **CORS is tested in
   stage 1**. Stage 2 adds `wss://`, mixed-content rules and the Cloudflare Tunnel (E7 idle behaviour, the "via public hostname"
   numbers). A page on the public GitHub Pages origin cannot reach a LAN address at all, so those runs use the public hostname.
2. The leaderboard module (E3): a ~10-line server module in this repo, `spikes/nakama_spike/server/`, deployed by bind-mounting a
   modules directory into the Nakama container (homelab task).
3. Until the homelab stack is ready, or for destructive tests, a throwaway local container of the **same version, 3.41.0** (the
   vendor's sample compose pins an older one); numbers must come from one version. Do not put that compose file in this repo.
4. Record at the start of every run: Nakama version, Postgres version, the effective `socket.*` / `session.*` / `matchmaker.*`
   settings, wired vs Wi-Fi for every client.

## Experiments

| ID | Experiment | What we learn |
|---|---|---|
| E1 | Device-id sign-in (create + re-login); **session lifetime (60 s) and refresh (refresh lifetime 3600 s); log in again after the refresh token expires; does an already open socket survive its token's expiry**; a socket is always connected with a freshly issued token | Effort of token handling in a long run; hidden failure mode of a 60 s token |
| E2 | Storage: write, read, optimistic-lock conflict; record the **exact HTTP status and body** of a stale-version write (3.41 changed the gRPC code) | Cloud-save semantics and error codes |
| E3 | Leaderboard: create it in a server module, submit a score as a client, list the top 10 | Real cost of "meta features need server code"; client-submitted scores are trusted (already known, recorded, not measured) |
| E4a | Relayed match: create (optionally `name`d), join by id and by name, leave | Effort of the socket protocol; match lifetime rules (a relayed match ends when its last member leaves and the id is not reusable) |
| E4b | Record what `ListMatches` actually returns for relayed matches | Whether a relayed match can serve as a lobby entry at all |
| E4c | One lobby-list approach that needs no server code: **public-read storage objects in a `lobbies` collection** | How `ListLobbies` would really be built |
| E5 | Relay latency and bandwidth (protocol below) | The number that decides whether a WebSocket relay is viable |
| E6 | Matchmaker: two clients add a ticket and get matched. **Matches form on a 15 s interval by default**, so "time to match" is recorded, not judged | Whether `FindMatch` can be built on it |
| E7 | Idle and drop behaviour (stage 2 for the tunnel): forced drop, client refreshes its session and reconnects, rejoins the same match if a member is still in it | Reconnect effort; whether the tunnel cuts quiet sockets (pings every 15 s keep it alive) |

### E5 protocol (revised)
**Calibration first**, to attribute any slow number: (C1) client-to-server round trip using the socket's own ping/pong (verify they
exist in `realtime.proto`), (C2) native-to-native relay, (C3) browser-to-browser relay, (C4) native-to-browser relay.

**Traffic design**
- **Load traffic** at the target rate to everyone in the match; **probe traffic** every Nth message addressed to **one** target only
  (recipient list), echoed back to the sender only, marked with an `op_code`. Never broadcast echoes (it multiplies load by clients-1).
- Schedule sends **by deadline** (`start + n * period`), not `sleep(period)`; timestamp each probe from its **scheduled** send time
  (avoids coordinated omission: stalls must not hide themselves by delaying the next send).
- Timestamp receives **inside the receive callback**, never at a 30 Hz main-loop poll (that adds up to 33 ms).
- Intervals use `Instant` (native) and `performance.now()` (browser), never wall-clock time.

**Metrics**: round trip is the main number. One-way delay is *derived* (NTP-style offset from the lowest-RTT echoes, assumes a
symmetric path), reported separately and **never used for go/no-go**. Also: longest gap between received messages (the TCP
head-of-line symptom), the achieved send rate (discard any run below 95% of target), `WebSocket.bufferedAmount` in the browser,
disconnects, and the server log's `queue full` / write-timeout lines. TCP delivers everything or drops the connection, so there is
no "packet loss" figure; reordering is only a sanity check.

**Run matrix** (the full cross product is 70+ runs and does not fit):
- **Main grid:** ~400 B snapshot (4 players of transform plus a few stats), 20 and 30 Hz, 2 and 4 clients, **JSON envelope**,
  on the LAN path, each configuration run **3 times** (report median and spread). Drop the first 5 s as warm-up.
- **Single runs:** 64 B and 1400 B at 30 Hz with 2 clients; the protobuf envelope on the main case only; one run from a **second
  device** (a phone browser on the LAN in stage 1; mobile data in stage 2); the "via public hostname" column in stage 2.
- Report **p99 only above ~2000 samples** (60 s at 10 Hz is 600 samples: p99 would be the 6th-worst value, noise).

**Stress and limits**
- Backpressure: with an outgoing queue of 16 and 4 clients at 30 Hz each client receives ~90 msg/s, so a hitch of ~180 ms can fill it.
  One deliberate test: pause the browser reader for 500 ms and record how it fails.
- Message size: the 1500-byte figure is guidance for authoritative matches; over a WebSocket the limit is `max_message_size_bytes`
  = 4096. 1400 B of payload is ~1.9 KB after base64 plus envelope. One probe at ~3-4.5 KB finds the real limit and how it fails
  (likely a disconnect): this decides whether snapshots need fragmenting.
- Back-of-envelope to sanity-check results: 4 clients x 30 Hz x ~700 B on the wire = ~63 KB/s into each client and ~250 KB/s
  (~2 Mbit/s) out of the server; residential upload is fine. Cut server-side bandwidth measurement unless the homelab exposes metrics.

**Browser driving**: a separate Playwright script `spikes/nakama_spike/run_browser.py` (not an entry in `test_web.py`), headed
Chromium launched with `--disable-background-timer-throttling --disable-renderer-backgrounding`, page kept visible (a background tab
throttles timers to 1 Hz and breaks E5 and E7). The native client creates or names the match and prints the id; the script opens
`index.html?match=<id>` served by `python serve.py 8002` (not 8000/8001, which other work uses), the page collects samples into
`window.__results`, the script writes the CSV. `serve.py`'s cross-origin isolation headers do not block CORS fetch or WebSocket.

**JSON traps** for the hand-written client: `int64` fields arrive as **strings** (`op_code`, `score`, probably more) and match data
is base64; serde must handle both or E1-E4 produce confusing errors. CPU comparison JSON vs protobuf at ~100 msg/s is noise: compare
sizes only. `prost-build` needs `protoc`, which Windows lacks: use `protox` (pure Rust) or generate once and commit the output.

## Tasks
- [x] S1 Prerequisites: endpoint reachable, version recorded (3.41.0), device-id sign-in works (checked with `curl` on the LXC)
- [x] S2a Scaffold `spikes/nakama_spike/` outside the workspace; protocol/transport split; native probe works
- [ ] S2b (`.gitignore` added) restructure into a protocol layer (no networking) and transport layer; wasm32 build
      of the protocol layer
- [ ] S3 E1 and E2 natively (needs the server key); then E1/E2 in a browser page (CORS from `http://localhost`)
- [ ] S4 E4a/E4b/E4c with `tokio-tungstenite` (native) and `ewebsock` (wasm); JSON envelope first
- [ ] S5 E3 once the homelab mounts a modules directory (module source in `spikes/nakama_spike/server/`)
- [ ] S6 E5: calibration runs C1-C4, then the main grid with deadline scheduling, probes and the Playwright script
- [ ] S7 E6 matchmaker; E7 drop/reconnect (tunnel part waits for the homelab stage 2)
- [ ] S8 Protobuf envelope on the main case (`protox`/`prost`); measure the protocol-layer line count and the wasm size with and
      without prost
- [ ] S9 Write the result: append "Spike result" to `planning/investigations/nakama_backend_viability.md`, state go / change-course
      against the criteria, estimate a maintained client crate, update the backlog items, delete or archive `spikes/nakama_spike/`

**Effort:** 1-2 days was optimistic. With the Playwright harness, the protobuf build and the calibration runs, plan **3-4 days**;
cut S8 if the JSON results are clearly good or clearly bad.

## Go / no-go criteria (written before running, per the review)
**Go** (write the `ironhold_backend` abstraction plan with a Nakama adapter) when all hold:
- A browser page connects over `ws://` (stage 1) and completes E1-E4 with no CORS workaround; over `wss://` (stage 2) the same,
  with no mixed-content workaround.
- E5, main grid (400 B, 20-30 Hz, 2-4 clients), **round trip: LAN p95 under 30 ms and p99 under 50 ms; public hostname p95 under
  150 ms and p99 under 250 ms.** (A relay round trip on a LAN should be single-digit to low-tens of ms; a looser budget would pass a
  broken setup.) **Stalls:** no gap between received messages above 200 ms on the LAN, at most one per minute via the public hostname.
- **Zero disconnects and zero `queue full` / write-timeout lines** in the server log during every main-grid run (TCP has no
  "sustained loss"; this replaces it).
- **Reconnect (E7):** after a forced drop the client refreshes its session and reconnects within 5 s, rejoining the same match if
  any member is still in it. Tokens: no failure in a 10-minute session without manual intervention.
- Protocol-layer size about **1000 lines or fewer** (counted for E1-E6 only), and the release `.wasm` size reported with and without prost.
- Operations checklist, not a feeling: Nakama and Postgres versions recorded, server log free of errors during the runs, effective
  configuration recorded.

**Change course** when:
- The relay misses the latency/stall/queue criteria: keep Nakama for meta services (E1-E3, E6) and use Matchbox/WebRTC for gameplay.
- The realtime protocol is impractical from Rust/wasm: use REST-only Nakama features and a separate small lobby service.
- Operating Nakama on the homelab is too fragile or exposed: consider Heroic Cloud, or a small own service for lobbies only.

## Progress log

**2026-10-24**
- S1 done. Endpoint reachable, Nakama **3.41.0**, device-id sign-in works (checked with `curl` on the LXC, tokens redacted); a wrong
  or default server key is rejected with HTTP 401.
- S2a done (native): standalone crate with `api.rs` (pure REST protocol), `config.rs`, `main.rs` (blocking native driver:
  `probe`, `e1`, `e2`). Probe from a dev machine over the LAN: `GET /` 200 in 26 ms (cold connection), sign-in with the default key
  401 in 7.6 ms.
- E1/E2 are written and wait on the server key file (`~/.nakama_spike_server_key`, outside the repo, never printed).
- Finding: leaderboards can only be created by server code, so E3 needs a module and a homelab modules mount.
- Plan review (system-architect, same day): verdict "needs more design work"; this revision applies it: E3 prerequisites, E4 split in
  three, native WebSocket via `tokio-tungstenite`, tighter latency budget plus stall/queue/reconnect criteria, calibration runs,
  probe/load separation, warm-up and repeats, trimmed matrix, stage-1 CORS correction, `.gitignore`, key/token handling, 3-4 day estimate.

## Open questions
- Does a socket survive its token's expiry? (E1 answers it; the plan assumes nothing.)
- Whether the socket's own ping/pong messages exist in `realtime.proto` for calibration C1 (verify first; otherwise use a probe echo).
- Whether the homelab will expose Nakama metrics (otherwise server-side bandwidth is not measured).

## Acceptance criteria
- Given the spike is complete, when the report is read, then it states the Nakama and Postgres versions, the effective server
  settings, the endpoints used (host names only, no LAN IPs), every E1-E7 outcome (including E4b's `ListMatches` result and E2's
  exact conflict status), the E5 numbers with the commands that produced them, and an explicit go / change-course decision.
- Given the decision is "go", when the report is closed, then a follow-up plan exists for the backend abstraction and the client
  crate, with an effort estimate based on the measured protocol-layer size.
- Given the spike is closed, when the repo is inspected, then `spikes/nakama_spike/` is gone or archived, no key or token is
  committed, no LAN IP appears in the report or the code (the endpoint comes from `NAKAMA_URL`), and no engine crate has changed.
