# Feature: Nakama backend spike (throwaway)

> **Code:** the spike crate (`spikes/nakama_spike/`) is deliberately not merged. It lives at git tag `spike/nakama_backend` (branch `feature/nakama_backend_spike`); check that tag out to rerun the stage 2 experiments.

_Status: Spike run, result written 2026-10-25 (investigation section 8c); stage 2 reruns open (started 2026-10-24; plan revised after a system-architect plan review the same day; see "Progress log")_
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
- [x] S6 E5 (native and browser calibration and grids, sizes and backpressure done; wired run, second device and stage 2 skipped or open, see the log and the spike result): calibration runs C1-C4, then the main grid with deadline scheduling, probes and the Playwright script
- [x] S7 E6 matchmaker; E7 drop/reconnect (done locally, see the log; the tunnel part waits for the homelab stage 2)
- [x] S8 Protobuf envelope on the main case (`protox`/`prost`); measure the protocol-layer line count and the wasm size with and
      without prost
- [x] S9 Write the result (section 8c of the investigation; the delete/archive of `spikes/nakama_spike/` waits for the decision): append "Spike result" to `planning/investigations/nakama_backend_viability.md`, state go / change-course
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

**SUPERSEDED, DO NOT USE: these two E5 sections (C2 and the first native grid) were measured with a harness bug, corrected in the
"E5 corrected" section below. tokio's timer on Windows fired sends up to ~15 ms late and the round trip is measured from the
scheduled send time, so that lateness was counted as relay latency (about +9 ms at p50, +11 ms at p95). Kept only as the record of
the mistake.**

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

**2026-10-25, E5 corrected (replaces the two superseded sections above).** Cause of the error: the native senders used tokio timers;
on Windows these can fire up to ~15 ms late and, because a probe is timestamped from its scheduled time, the lateness was added to
the round trip. It showed up when the first browser run (browser timers are 1 ms) came out far *faster* than native. Fix: the native
senders are OS threads that sleep most of the interval and spin the last millisecond; the harness now prints the send lateness
against the deadline (mean 0.01-0.04 ms, a few runs with a single late send up to 24 ms). The browser harness reports its own send
lateness, which is Chromium's `setTimeout` (mean 1.6-3.8 ms) and therefore *is* included in browser round trips. Wi-Fi 6E to the
homelab for all runs below; Nakama 3.41.0; release builds; 5 s warm-up dropped; round trip = A -> server -> B -> server -> A.

Calibration (probes only, 30 Hz, 64 B, 90 s each, zero disconnects):

| Run | n | p50 | p95 | p99 | max | longest receive gap | send lateness mean / max |
|---|---|---|---|---|---|---|---|
| C2 native <-> native | 5100 | 5.01 | 11.57 | 19.59 | 34.2 | 55 ms | 0.01 / 5.2 ms |
| C3 browser A <-> browser B (page A) | 2550 | 6.20 | 15.87 | 22.57 | 34.9 | 50 ms | 1.57 / 13.1 ms |
| C3 (page B) | 2550 | 8.23 | 20.50 | 26.57 | 40.2 | 55 ms | 3.83 / 16.7 ms |
| C4 native host, probes to the page | 2550 | 4.80 | 11.28 | 18.90 | 31.9 | 49 ms | (native) |
| C4 browser page, probes to the native host | 2550 | 6.47 | 15.50 | 25.17 | 42.6 | 70 ms | 2.68 / 10.6 ms |

- The relay path itself is cheap: ~5 ms median for two server hops and two Wi-Fi hops each way. A browser adds about 1.5-3.5 ms (its
  timer lateness plus the callback) and widens the tail a little; it does not change the picture. The wasm module is 355 KB
  (release, protocol layer plus `ehttp`, `ewebsock`, `serde_json`, no protobuf yet).
- All five runs fit the LAN budget (p95 < 30, p99 < 50, gaps < 200 ms).

Native main grid (400 B, JSON envelope, 65 s per run, probes 10 Hz on top of the load, 3 repeats per cell; p99 only where n >= 2000):

| Clients | Load per client | p50 | p95 | p99 | worst single sample | longest receive gap |
|---|---|---|---|---|---|---|
| 2 | 20 Hz | 5.1-5.2 | 11.2-17.0 | n/a (n=1200) | 26-336 | 64, 65, **352** ms |
| 2 | 30 Hz | 4.8-5.1 | 9.9-12.5 | n/a | 29-35 | 50-52 ms |
| 4 | 20 Hz | 9.5-9.6 | 15.0-18.3 | 26.1-32.0 | 35-77 | 68-98 ms |
| 4 | 30 Hz | 9.5-9.7 | 14.9-22.0 | 23.0-36.0 | 37-143 | 54-113 ms |

- Zero disconnects or errors in all 12 runs; send rate 100% of target; writer queue drained; the server log (checked after the
  calibration and before the grid) shows only startup lines.
- p50 and p95 are far inside the budget (p95 at most 22 ms against 30). Going from 2 to 4 clients adds about 4.5 ms at p50 (the
  server forwards more messages); 20 vs 30 Hz makes no measurable difference.
- **One stall breaks the "no gap above 200 ms on the LAN" criterion:** rep 3 of the 2-client / 20 Hz cell had a 352 ms gap and a
  336 ms worst round trip; the other two 4-client cells of that repeat also had their worst samples (77 and 143 ms). Every run
  before rep 3 stayed under 78 ms. The cause is **not attributed**: a Wi-Fi hiccup (more likely, the pattern appeared in the last
  third of the 14-minute grid and nowhere else), a server GC pause or TCP head-of-line blocking after a retransmit are all
  possible. A wired run and a server-side log/metrics look are needed before this counts for or against the relay. One stall in
  about 12 minutes of traffic is also the kind of event that decides whether snapshot-style state replication (tolerates a stall) or
  input-style (does not) is the right use of this relay.

### 2026-10-25, E5 main grid from browser pages (Wi-Fi, no wired run available)

1 native host + 1 or 3 Chromium pages (visible windows, throttling switches), 400 B, JSON envelope, 65 s per run (5 s warm-up
dropped), probes 10 Hz per client on top of the load, 3 repeats per cell, host `192.168.178.106:7350`. Page figures are pooled over
the pages (probe RTT from the scheduled send time, so the page's own `setTimeout` lateness is inside them; send lateness mean was
0.6-5.6 ms). p99 is not reported: at most 1800 pooled samples, under the 2000 rule.

| Clients / load | page p50 | page p95 | page worst | longest page receive gap | native host p95 |
|---|---|---|---|---|---|
| 2 / 20 Hz (3 reps) | 6.0-10.7 | 14.6-18.4 | 32.7-40.7 | 104-116 ms | 8.1-15.7 |
| 2 / 30 Hz | 5.5-6.3 | 10.2-19.0 | 30.3-34.5 | 101-106 ms | 8.3-10.3 |
| 4 / 20 Hz | 12.7-15.0 | 24.4-27.7 | 38.6-69.6 | 59-69 ms | 10.5-14.2 |
| 4 / 30 Hz | 7.3-8.0 | 18.0-21.0 | 33.4-45.7 | 42-57 ms | 10.2-11.3 |

- 12 of 12 runs: zero disconnects, zero socket errors, no sample or gap near 200 ms (largest gap 116 ms, worst RTT 69.6 ms).
  **The 352 ms stall did not reappear**, which supports (does not prove) the Wi-Fi explanation for the native one.
- Page p95 is inside the 30 ms budget in every run, but 4 clients / 20 Hz reached 27.7 ms: little headroom there. That cell is the
  one with the highest page send lateness (mean about 5 ms), so part of it is the harness, not the relay.
- The ~105 ms receive gap in every 2-client run is constant across runs, so it is probably a harness or schedule artefact, not
  network jitter (not investigated; well under the 200 ms criterion).
- Still open for E5: message sizes (64 B / 1400 B / ~4 KB), the backpressure test, a second device, a wired run (skipped, no cable).

### 2026-10-25, E5 message sizes and backpressure (native, Wi-Fi, host 192.168.178.106)

**Message size** (2 clients, load 20 Hz, probes 10 Hz, 35 s, JSON envelope; payload is the raw bytes, sent as base64 inside JSON):

| payload | RTT p50 | p95 | max | result |
|---|---|---|---|---|
| 64 B | 5.3 | 11.3 | 21.8 | ok |
| 1400 B | 9.4 | 14.5 | 28.0 | ok |
| 2800 B | 13.9 | 22.7 | 32.1 | ok |
| 4000 B | - | - | - | **both sockets closed with WebSocket 1009 (message too big) on the first send** |

- Under a 2-client, 20 Hz load the median RTT grew with payload size (5 -> 9 -> 14 ms). **Corrected by E8 below:** with one
  probe in flight and no load, the same sizes cost the same 5 ms, so the growth above was queueing/airtime under load (Wi-Fi), not
  a per-byte cost of JSON or base64. Larger packets matter because they are sent at a rate, not because of their size alone.
- **Effective payload ceiling is about 2.9 KB, not 4 KB:** Nakama's 4096 byte limit applies to the whole socket message and base64
  inflates the payload by a third, plus the envelope. A protobuf envelope (S8) has the same base64-free advantage only for the
  envelope itself, so S8 should also measure the ceiling. Anything larger needs splitting, delta compression or a storage/RPC path.

**Backpressure** (4 clients, load 30 Hz, 2800 B payload, probes 10 Hz, 45 s; `E5_STALL_MS` makes the last client stop reading once,
10 s after the warm-up; about 340 KB/s flows towards it):

| stall | outcome |
|---|---|
| 0 ms | baseline: p50 15.7, p95 25.7, max 39.2, 0 disconnects |
| 500 ms | no loss (4403 of 4403 messages delivered after resume), the stalled client's longest gap 501 ms, worst RTT 517 ms (its own echoes), other clients unaffected apart from one 117 ms gap |
| 3000 ms | **the server dropped the slow client** (connection forcibly closed by the remote host); the other clients kept receiving at full rate |
| 10000 ms | same drop |

- A 500 ms freeze (tab throttled, GC, a long frame) is absorbed by TCP and the server queue with no loss and no effect on others;
  3 s is not, at this data rate. **Server log (read afterwards):** for the stalled session only, a burst of `Could not write message,
  session outgoing queue full` and `Failed to route message ... session outgoing queue full`, then `Could not write message ... write
  tcp ...: i/o timeout` and the close. So the mechanism is: the client stops reading, TCP backs up, the server's per-session outgoing
  queue fills, messages to that session are dropped, and the write timeout closes it. Other sessions were not affected. The exact
  threshold is not established (it depends on the data rate). Clients must treat a drop as normal and reconnect (E7).
- **Log check for the "zero `queue full` / write-timeout lines" criterion:** over the last 3 hours the server log has 1177 such
  lines, **all in the single minute 10:33 UTC, which is the 3 s stall run**. No other minute has one, so every native and browser
  grid run met the criterion. The lines appear only for a client that stops reading.
- A dropped member also removes its ring-probe target, so the client before it in the ring loses its RTT samples (harness effect).

### 2026-10-25, E6 matchmaker, E7 drop/reconnect/soak, E8 protobuf (native, Wi-Fi)

**E6 (`nakama_spike e6`):** two clients each add a ticket (min 2, max 2, query `*`) and both are told they matched **about 9.8 s
later** (the interval is a server setting, default 15 s; the match formed on the next tick). The result carries a join `token` and no
match id; both clients join the same relayed match with `match_join {token}` (4 ms) and data flows. `FindMatch` can be built on it.
Time to match is bounded by the matchmaker interval, so a UI needs a "searching" state of at least that long unless the interval is
lowered in the server config.

**E7 (`nakama_spike e7`, `e7 soak`):**
- After the socket is dropped the client refreshes its session, reconnects and rejoins **the same match in 13 ms** (criterion: 5 s);
  the other member sees the leave and the rejoin; data flows. The server also noticed the drop within 2 ms, **but that is a clean TCP
  close** from dropping the socket in-process. A half-open drop (pulled cable, lid closed) is detected only by the server's ping
  timeout; not measured here.
- A connect with an expired token (60 s lifetime, waited 65 s) is refused with **HTTP 401**; after a session refresh it works. A
  client must refresh before reconnecting.
- **Soak, 600 s:** two clients in a match, one targeted message per second, a socket ping every 15 s, a session refresh every 45 s
  (13 refreshes): 600 of 600 messages delivered, slowest 76 ms, no disconnect, no manual step. Meets "no token failure in a
  10-minute session". The tunnel and idle-socket behaviour (stage 2) is still open.

**E8 / S8, protobuf envelope (`--features proto`, `nakama_spike e8`):** hand-written `prost` structs for the used subset with the
field numbers of nakama-common `realtime.proto` (what `prost-build` would emit; no `protoc` needed). 300 sequential round trips per
cell, one in flight, no load:

| payload | JSON wire / RTT p50 p95 | protobuf wire / RTT p50 p95 |
|---|---|---|
| 400 B | 789 B / 4.8, 11.4 | 539 B / 4.7, 14.2 |
| 2800 B | 3989 B / 5.2, 10.2 | 2939 B / 5.0, 14.6 |
| 3900 B | **fails** (over the 4096 byte limit, socket closed) | 4039 B / 5.3, 15.4 |

- Latency is the same for both formats (p95 differences are noise at n=300 on Wi-Fi). Protobuf saves bandwidth (about 30% at
  400 B, the 400 B payload plus about 140 B of presence/ids) and raises the **payload ceiling from about 2.9 KB to about 4 KB**.
- Cost: release wasm **369,298 B (JSON only) vs 403,172 B with prost** (+33.9 KB, +9%); protocol layer **486 lines** (JSON, incl. REST
  and matchmaker) **+ 178 lines** for the protobuf subset = 664, under the 1000-line criterion. Unit tests included in the counts.
- The binary socket works with `?format=protobuf` and the browser transport (`ewebsock`) already handles binary frames, but the
  protobuf path was only exercised natively here.
