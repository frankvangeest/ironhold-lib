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
- [x] S2b restructure into a protocol layer (`src/protocol/`, no networking) and transport layer (`src/native.rs`); the library
      checks for `wasm32-unknown-unknown`, 3 unit tests on the socket envelope pass
- [x] S3a E1 and E2 natively (2026-10-24, results in the Progress log)
- [x] S3b E1/E2 in a browser page served from `http://localhost:8002` (CORS preflight with the Basic-auth header): passes
- [x] S4a E4a/E4b/E4c natively with `tokio-tungstenite` (2026-10-24, JSON envelope; results in the Progress log)
- [x] S4b socket open, ping and a relayed match from a browser page with `ewebsock` (first part of E4; two-client parts in E5)
- [x] S5 E3 with a Lua module (`spikes/nakama_spike/server/spike_leaderboard.lua`): passes
- [ ] S6 E5 (C1 and C2 done natively, see the log; C3/C4 need the browser side): calibration runs C1-C4, then the main grid with deadline scheduling, probes and the Playwright script
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

**2026-10-24, E1 and E2 results (native, dev machine on Wi-Fi to the homelab LXC, Nakama 3.41.0; times are one-shot REST calls, not a
benchmark):**
- E1 sessions: device-id sign-in create 9-22 ms; re-login of the same device returns the same account (`created=false`); refresh 8-9 ms;
  a garbage refresh token is rejected with 401. **Session token lifetime is 60 s** (the default, as predicted): every REST call in a
  longer run needs a refresh step. Still open: does an open socket survive its token's expiry; log in again after the 3600 s refresh
  token expires (needs a long run).
- E2 storage: write 13-14 ms, read 8-14 ms. The stale-version write is rejected with **HTTP 400** and body
  `{"code":9, "message":"Storage write rejected - version check failed."}` (the 3.41 FAILED_PRECONDITION code, mapped to 400), and a
  create-only write (`version=*`) on an existing key is also HTTP 400. Optimistic locking works as assumed.
- **Finding: stored JSON is normalized.** The value read back has the same content but not the same text (key order and spacing
  differ; Postgres `jsonb` semantics): a save file must not rely on byte-exact or key-ordered round trips.
- **Observation: the object `version` looks content-derived.** Two different users writing the same document got the same
  version prefix. So a version identifies content, not a write; two writers that happen to store identical content do not conflict.
  (To confirm: compare the full version strings.)

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

**2026-10-24, E4 results (native `tokio-tungstenite` with NODELAY, JSON envelope, two users; one-shot times, not a benchmark):**
- Socket connect 6 ms; a socket ping/pong round trip (calibration C1) 11.5 ms on this first sample. Creating a relayed match 4.6 ms,
  joining by id 2.3 ms, a broadcast reaching the other member 1.4 ms (a single sample, so the real distribution comes from E5).
- E4a: broadcast and targeted (recipient list) sends both arrive intact; `op_code` arrives as a string and the data as base64, as
  predicted; the sender does **not** get its own broadcast back (so a probe needs a second client or a targeted echo). The other
  member sees join and leave through `match_presence_event`. **A relayed match is gone when its last member leaves**: joining its id
  afterwards fails with `4: Match not found`. Creating a match with the same `name` twice returns the **same match id** (a name is a
  stable room key; the second creator joins, size 2).
- **E4b: ListMatches does list relayed matches** (`authoritative=false`): the entry has only `match_id` and `size`, no label. The
  docs gave the opposite impression. A relayed match can therefore appear in a list, but with no title, mode or capacity, so it is
  not enough for a lobby on its own. `authoritative=true` lists nothing here.
- E4c: a public-read (`permission_read` 2) storage object in a `lobbies` collection is visible to another user through
  `GET /v2/storage/lobbies` (list across owners, no server code). It lets a lobby carry host name, mode, player count and the match
  id; the cost is that stale entries have to be removed by the owner or a server job (nothing expires them).
- Protocol layer after E1-E4: about 400 lines (rest 177, socket 223, mod 5), counted again at S9.

**2026-10-24, E3 and the first browser run (homelab, Nakama 3.41.0, one-shot times):**
- E3: the Lua module (6 lines of logic, no build step) creates the leaderboard `spike_best` at server start. Three clients submitted
  scores (HTTP 200, 10-15 ms each) and the top-10 listing returned them best first (`450, 300, 120`). A client can submit any value
  (999999999 was accepted with HTTP 200), so scores are trusted unless the leaderboard is made authoritative, which needs more
  server code. The module reached the server by copying a file into the bind-mounted directory and restarting the container (one
  restart, no rebuild).
- Browser (headed Chromium, page on `http://localhost:8002`, Nakama on another origin over plain `http://` and `ws://`): sign-in
  (cross-origin POST with an `Authorization` header, so a CORS preflight), refresh, storage write (PUT) and read, `ewebsock` socket
  open (17 ms), socket ping (11 ms), relayed match create (5 ms) and leave all work, with **no CORS workaround and no server
  setting changed**. That meets the first go criterion for stage 1 (`wss://` and mixed content wait for stage 2). The wasm module
  is the same protocol layer as the native driver; only the transport differs.

**2026-10-24, E5 calibration C2 (native to native, release build, 2 clients, probes only at 30 Hz, 64 B, 90 s, 5 s warm-up dropped;
dev machine on Wi-Fi 6E to the homelab, Nakama 3.41.0):**
- Round trip A -> server -> B -> server -> A (probe out, echo back), timestamped from the scheduled send time: **n=5100, min 4.3,
  p50 14.2, mean 14.4, p95 22.3, p99 32.1, max 46.4 ms.** The LAN budget (p95 < 30, p99 < 50) is met on Wi-Fi already; a wired run
  should only tighten it. For scale: the socket's own ping/pong was 11.5 ms in the single E4 sample.
- Longest gap between received messages 62 ms (probes arrive at 30 Hz, so the normal gap is 33 ms): no stall near the 200 ms limit.
- Send rate 100% of target, writer queue fully drained, **zero disconnects over 90 s**, which is past the 60 s session token: an
  **open socket survives its token's expiry** (answers the open E1 question for this case; reconnecting after expiry still needs a
  fresh token, that is E7).
- Not yet checked: the Nakama server log for `queue full` / write-timeout lines for this run (needs `docker logs` on the LXC).

**2026-10-24, E5 main grid, native (release build, 400 B, JSON envelope, 65 s per run with 5 s warm-up dropped, probes 10 Hz on top of
the load, 3 repeats per cell; Wi-Fi 6E to the homelab, Nakama 3.41.0). Round trip A -> server -> B -> server -> A, ms:**

| Clients | Load per client | p50 (range of 3) | p95 (range of 3) | p99 (range of 3, n=2400 per run) | Worst single sample | Longest receive gap |
|---|---|---|---|---|---|---|
| 2 | 20 Hz | 14.1-14.8 | 22.0-24.8 | n/a (n=1200) | 50.9 | 77 ms |
| 2 | 30 Hz | 14.0-14.9 | 22.1-22.9 | n/a (n=1200) | 45.4 | 71 ms |
| 4 | 20 Hz | 17.6-18.4 | 26.4-27.0 | 31.0-34.5 | 44.9 | 76 ms |
| 4 | 30 Hz | 17.9-18.6 | 26.4-27.7 | 32.5-35.5 | 47.5 | 62 ms |

- All 12 runs: send rate 100% of target (30 Hz runs 100.1%), writer queue drained, **zero disconnects or errors**.
- Against the LAN criteria (p95 < 30, p99 < 50, gaps < 200 ms): **met in every cell**, but 4 clients leave only about 2.5-3.5 ms
  of p95 headroom on Wi-Fi, so the margin there is thin. Going from 2 to 4 clients costs about 4 ms at p50 and 4-5 ms at p95; going
  from 20 to 30 Hz changes nothing measurable. The relay's cost is dominated by the two server hops, not by the send rate.
- Outliers: no single round trip above 51 ms in 15,600 probe samples.
- Still to do for the verdict: the Nakama log for `queue full`/write-timeout lines (a check after the 90 s calibration run found none,
  only startup lines), a wired run for the Wi-Fi caveat, and the browser-side numbers (C3/C4 plus the grid from a page).
