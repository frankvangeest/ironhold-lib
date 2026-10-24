# Investigation: Nakama as the game backend (build our own vs adopt vs both)

_Status: Draft for decision (nothing built)_
_Opened: 2026-10-23, `58a6187`_
_Question from Frank:_ is Nakama (open-source game backend, runnable from a container in the homelab) viable for Ironhold;
would it be easier to write our own; what can and can't Nakama do; start small on our own, adopt Nakama, or support both?

Related plans this touches: `planning/features/networking_multiplayer.md` (Forms 1-3),
`docs/dev/40_determinism_and_networking.md`, backlog "Beta 0.6 / 0.8 / 0.9+", `planning/investigations/bevy_019_upgrade.md`.

## 1. TL;DR

- Nakama is **not a replacement for our in-game networking**. It does not run a Bevy simulation, replicate entities, predict or
  roll back. It is the *meta* backend: accounts, cloud saves, leaderboards, friends, chat, lobbies, matchmaking, notifications,
  and a message relay. We need a Rust-native sync layer either way.
- Nakama **is** a strong fit for everything around the game that is boring, security-sensitive and expensive to build well
  (auth, storage, leaderboards, lobbies, matchmaking). Building those ourselves is the larger job.
- The costs of adopting it are real but bounded: **no maintained Rust client** (we write a thin one), server logic extensions are
  **Go / TypeScript / Lua** (a second language, and a tension with "designers write no code"), **clustering and HA are
  Enterprise-only** (a single node is fine for our scale), and the authoritative-match model is capped at **60 ticks/s and
  ~1500-byte messages** and cannot host our simulation.
- **Recommendation: support both, layered.** Keep the Rust-native sync path for gameplay (Form 1 needs no backend at all). Put
  accounts / storage / leaderboards / lobbies / matchmaking behind a small engine-side **backend abstraction** whose first
  (and probably only) adapter is Nakama. Do **not** build our own account/storage/leaderboard service. Before any commitment run a
  **1-2 day throwaway spike** (section 8) with explicit go/no-go criteria.

## 2. What we need (from the existing plans)

| Need | Where it comes from | Backend involved? |
|---|---|---|
| LAN co-op, one player hosts | Form 1 (Beta 0.6) | **No.** Direct connection, no service |
| Internet play incl. browser (WASM) clients, lobby list | Form 2 (Beta 0.8): `CreateLobby` / `JoinLobby` / `ListLobbies` | Relay/signaling + lobby registry |
| Headless authoritative server running `ironhold_core` | Form 3 (Beta 0.9+), new `ironhold_server` crate | Allocation / discovery of servers |
| Accounts, cloud saves, scores, achievements | Not planned yet, but any shipped game wants them | **Yes** (the big unplanned cost) |
| Works from a static GitHub Pages build | Current demo hosting | Backend must be **optional**: the game keeps working with it absent |

Constraints that decide a lot: browser clients can only use **WebSocket / WebRTC / WebTransport**; `ironhold_core` must stay
platform-agnostic; the engine's principle is that **designers author in RON and never write Rust**; the sim runs a fixed
**64 Hz** tick and lockstep depends on the unfinished cross-platform determinism harness.

## 3. What Nakama is (verified)

Sources are listed in section 10. "Verified" means read on the vendor's own pages or repo; vendor claims are marked.

- **License / cost:** server is Apache-2.0. Self-hosting costs only your own infrastructure. A Heroic Labs forum answer
  (2021) says the open-source build is "not feature limited or restricted in any way" and can run a single-server production
  deployment, and that no CockroachDB Enterprise licence is needed.
- **Database:** CockroachDB or any Postgres-wire-compatible database. The current Docker setup pins `postgres:16.8-alpine` and a
  Nakama 3.37 image. One database to operate.
- **Activity:** latest release is v3.41.0 (18 Sep 2026; the year is confirmed by the Docker Hub tag date), preceded by
  3.40 (13 Jul), 3.39 (20 May), 3.38 (20 Mar), 3.37 (12 Feb): a release roughly every two months, actively maintained.
- **Feature set (README):** authentication (device id, email, and social providers such as Facebook, Google, Steam and GameCenter per the vendor page; the full current list was not checked), user storage
  in collections, friends / groups / social graph, chat with persisted history, leaderboards (dynamic, seasonal),
  tournaments, parties, realtime or turn-based multiplayer (relayed and authoritative), matchmaker, in-app notifications,
  purchase validation, console dashboard, metrics.
- **Server extensions ("runtime"):** Go (compiled plugin), TypeScript/JavaScript, or Lua.
- **Protocols:** gRPC, REST (HTTP/1.1 + JSON), WebSocket for realtime, rUDP per the README. The realtime socket can use JSON or
  protobuf (seen in third-party/forum material; the authoritative definition is `realtime/realtime.proto`; **not verified
  first-hand**).
- **Official clients:** .NET, Unity, JavaScript, Java/Android, Unreal, Godot, Defold, Swift/iOS. **No Rust client.**
  `heroiclabs/nakama-rs` exists but is **archived and read-only since 23 Nov 2024**; its README covers the REST-style API only
  (no realtime socket, no matches, no matchmaker mentioned) and does not claim WASM support.
- **Multiplayer models:**
  - *Relayed*: clients send messages through Nakama to each other; no server-side game logic. Fits a player-hosted listen server.
  - *Authoritative*: you write a **match handler** (7 callbacks: init, join attempt, join, leave, loop, terminate, signal) in
    Go/TypeScript/Lua; Nakama calls the loop at a fixed rate. Documented limits: **tick rate 1-60**, keep messages within the
    **1500-byte MTU**, aim for **at most one message per tick per presence per direction**, match labels 2 KB, a match runs on
    **one node**, loops that fall behind end the match. "Migrating players during shutdown" is **Enterprise-only**.
  - *Session-based (dedicated servers)*: Nakama does not simulate; it orchestrates headless servers through a **FleetManager**
    interface (list / join / create / get / delete). Open-source implementations exist for Amazon GameLift (Heroic), Edgegap and
    i3D.net (each vendor), plus a community Gameye one. **No Agones implementation is shipped** (a forum reply invites a
    community one); the docs say this model is for "complex physics/graphic-laden games", which is the Form 3 case.
- **Scaling:** open-source runs as a **single node**. **Clustering, HA and multi-node presence are Nakama Enterprise**
  (architecture docs). A Heroic blog (vendor claim, not measured by us) estimates ~10,000 concurrent connected users per node
  and recommends at least 2 nodes in production for failover; a homelab single node is fine for a hobby or demo audience but
  has no failover. Heroic Cloud (managed) and the Hiro / Satori products (economy and LiveOps) are paid Heroic offerings; their
  prices were not available in what was read.
- **Anecdote, not a limit:** one forum user reports trouble beyond ~15 players each sending 20 state updates/s in one
  authoritative match. Treat as a single data point; the official benchmark page could not be read in this investigation.

## 4. What Nakama can NOT do for us (the important half)

1. **It cannot run our simulation.** Match handlers are Go / TypeScript / Lua. `ironhold_core` is Rust on Bevy with Rapier. An
   "authoritative match" in Nakama would mean re-implementing gameplay rules in another language and keeping it in sync with the
   engine: unacceptable duplication for a data-driven engine whose rules live in RON. The only route to a server-side
   simulation is a headless `ironhold_server` process that Nakama allocates (session-based), which we must build anyway.
2. **No state replication, interpolation, prediction or rollback.** It moves bytes. Everything in Forms 1-3 about replicated
   components, input buffering and lockstep stays our job, with a Rust sync library (Lightyear, bevy_replicon + a transport,
   matchbox, ...). Which of those support our Bevy version was **not verified here** (see section 9).
3. **60 ticks/s and ~1500-byte messages.** Our fixed tick is 64 Hz. Network rate need not equal sim rate (a 30 Hz network
   tick is typical), so this is a design constraint, not a blocker, but large snapshots would have to be fragmented by us.
4. **Relay over WebSocket means TCP.** Head-of-line blocking hurts fast action games on lossy links. Fine for lobbies,
   turn-based and co-op at moderate tempo; worse than WebRTC unreliable channels (which Matchbox offers) or WebTransport.
5. **No maintained Rust client.** We would write one: REST (JSON, any HTTP client incl. wasm `fetch`) is easy; the realtime
   socket needs the protobuf (or JSON) `Envelope` implemented with `prost`/`serde_json` plus a wasm-capable WebSocket.
   Rough size: a few hundred lines for auth + storage + leaderboard + lobby + match relay (our estimate, to be confirmed by the spike).
6. **Server extensions are code.** Anything beyond configuration (custom lobby rules, score validation, rate limits) is written in
   Go/TS/Lua. That collides with "designers need no Rust / no code" unless **we ship one generic module** (a TypeScript runtime
   package, e.g. `ironhold-nakama`) that exposes data-driven behaviour, and designers only choose options in RON.
7. **No HA / clustering in the open-source build.** Single point of failure; backups and upgrades are ours. A residential
   homelab adds uptime, dynamic-DNS and TLS concerns (browsers need `wss://` from an `https://` GitHub Pages origin, plus CORS).
8. **Not a determinism or anti-cheat solution.** A relayed match trusts the host/clients. Only an authoritative simulation fixes that.

## 5. Build our own vs adopt Nakama

### What "our own" realistically means
Two very different scopes:

- **Own small relay/lobby only** (Form 2): a signaling/relay service plus a lobby registry. Our existing plan already picks
  Matchbox (~1 container). Small, and it needs no database.
- **Own full backend** (accounts, storage, leaderboards, friends, chat, matchmaking, notifications): a Rust service
  (e.g. Axum + sqlx + Postgres) with password/OAuth handling, sessions, rate limiting, abuse handling, migrations, an admin
  UI, backups. This is a product. It is also exactly where homemade code is most likely to ship a security bug.

### Comparison

| Aspect | Own (small relay + lobbies) | Own (full backend) | Nakama |
|---|---|---|---|
| Time to first working lobby | Short | Long | Short once the client exists (spike) |
| Accounts / saves / leaderboards | Not provided | We build and secure them | Provided |
| Matchmaking, friends, chat, notifications | Not provided | We build | Provided |
| Language / stack fit | Rust, one stack | Rust, one stack | Go/TS/Lua server, Rust client we write |
| WASM browser clients | Matchbox: WebRTC, supported | We choose transport | WebSocket (REST+socket); official JS client proves it works |
| Designer-facing (RON, no code) | Natural: our Actions map 1:1 | Natural | Needs one shipped generic runtime module |
| Ops burden | One container, no DB | Server + DB + backups + upgrades + security | Nakama + Postgres, upgrades every ~2 months, backups |
| Security surface | Small | Large, ours | Large, but battle-tested and maintained by others |
| Scale / HA | Whatever we build | Whatever we build | Single node OSS; clustering is Enterprise |
| Lock-in / exit | None | None | Apache-2.0 server, our data in Postgres; protocol coupling lives in our adapter |
| Bus factor / vendor risk | Ours | Ours | Heroic Labs (company); Apache-2.0 allows forking |
| Cost | Our time | Mostly our time | Our time + a Postgres; money only if we go Enterprise/Cloud |

### Judgement
- For **in-game sync**, "own" is not optional: no backend does it for us. That part is the same in every option.
- For **meta services**, adopting is clearly cheaper and safer than building, *if* the thin Rust client and the "designers
  write no code" tension are acceptable (both look manageable).
- For **Form 2 relay**, Nakama's relayed match is a credible alternative to Matchbox (it also removes NAT traversal and TURN
  concerns because everything goes through the server), at the cost of TCP latency and relay bandwidth through the homelab. Matchbox
  (WebRTC) is better for latency-sensitive play. Both can exist behind the same abstraction.

## 6. Other questions worth deciding (checklist)

1. **Optionality.** The game must run with no backend (GitHub Pages, LAN, single player). Backend features must degrade
   gracefully and never block start-up. This should be an engine rule.
2. **Where does the abstraction sit?** Candidate: RON Actions (`SignIn`, `SaveCloud`, `SubmitScore`, `CreateLobby`,
   `JoinLobby`, `FindMatch`) and pipeline events (`backend.signed_in`, `lobby.joined`...) in `ironhold_core`, with an
   `ironhold_backend` crate (or feature) holding the Nakama adapter. `ironhold_core` stays platform-agnostic.
3. **Designer experience.** What does a designer configure (RON) vs. what ships as a generic server module? Who deploys it
   (docs for deployers, `docker-compose.yml` template)?
4. **Trust model per form.** Form 1/2 host-authoritative (host can cheat), Form 3 server-authoritative. Is leaderboard
   score submission trusted from clients? (Needs server-side validation: Nakama runtime code.)
5. **Privacy and compliance.** Accounts mean personal data (email, device id). If the demo is public from the homelab
   (EU/NL), GDPR duties apply: deletion, export, retention. whether Nakama offers the account deletion and export hooks we would need was not checked; we own the policy either way.
6. **Operations.** TLS (reverse proxy, certificates), dynamic DNS/port-forward or a tunnel, backups of Postgres, update cadence,
   monitoring, changing the default `admin/password` console and the server key, rate limiting.
7. **Protocol/version coupling.** Our hand-written client must track Nakama API changes (e.g. v3.41 changed a gRPC status
   code for storage version conflicts). Pin the server version; keep contract tests against a Nakama container in CI.
8. **Interaction with determinism work.** Lockstep over a TCP relay needs the determinism harness and fixed-tick logic first
   (backlog, Beta 0.5). A backend does not remove that dependency.
9. **Bevy upgrade.** The sync library choice must follow the Bevy upgrade (0.19 is out per release news;
   `planning/investigations/bevy_019_upgrade.md`). Pick the sync stack after that decision, not before.
10. **Testing.** Integration tests against a real Nakama container (docker) in CI; a fake in-memory backend for unit tests and
    for the "no backend" path.
11. **Cost of leaving.** If we later outgrow Nakama (or the company changes terms), the abstraction must let us swap adapters.
    The data lives in our Postgres and is exportable.
12. **Scope discipline.** Which meta features do we actually want in the first year? Probably: sign-in (device id first),
    cloud save, one leaderboard, lobby list. Not: IAP, tournaments, chat, parties.

## 7. Options and recommendation

| Option | What it means | Verdict |
|---|---|---|
| A. Own only, start small | Matchbox relay + hand-built lobby registry; add accounts/saves later | Fastest for Form 2 alone, but the meta-services cost is deferred, not avoided |
| B. Nakama only | Everything through Nakama, including relayed gameplay | Works for casual/co-op, but it ties gameplay transport to a 60 Hz TCP relay and a second language |
| C. **Both, layered (recommended)** | Rust-native sync (own) for gameplay; engine-side backend abstraction with a Nakama adapter for meta + lobbies/matchmaking + optional relay | Best fit: each tool does what it is good at, nothing blocks offline/LAN play |

Why C:
- It keeps Form 1 backend-free and keeps the sync layer replaceable (Rust, our determinism rules).
- It spends our time on engine features, not on password storage and leaderboards.
- The abstraction is the exit strategy and the offline fallback in one.
- It does not commit us before evidence: the spike (below) decides whether the adapter is worth writing.

What to **avoid**: using Nakama's authoritative match handlers for simulation; building our own account/storage service;
writing designer-facing server code in Go/TS.

## 8. Proposed spike (1-2 days, throwaway, no engine changes)

_Now planned in `planning/features/nakama_backend_spike.md` (experiments, measurement protocol, go/no-go). Hosting the server is
tracked in the homelab repo (`planning/features/nakama-deployment.md`), not here._

Goal: replace opinions with evidence before any plan or backlog commitment.

1. Run Nakama + Postgres in the homelab container; put `wss://` in front of it; change default credentials and server key.
2. A standalone Rust test binary (native **and** wasm32 build, not in `ironhold_core`) that: authenticates with a device id,
   writes and reads a storage object, submits and lists a leaderboard score, lists/creates a **relayed** match and exchanges
   messages between two instances (one native, one browser tab served from the GitHub Pages origin).
3. Measure: round-trip latency from the browser over the homelab link, message size and rate headroom at 20/30 Hz with a
   realistic snapshot (`Transform` + `StatMap` for 4 players), CORS/TLS pitfalls, client code size.
4. Write down the effort to turn this into a maintained client crate.

**Go criteria:** browser client connects over `wss` from `https`; latency and bandwidth acceptable at 20-30 Hz for 2-4 players;
thin client under ~1000 lines for the features above; no licence or database surprise.
**No-go / change course:** WebSocket relay too laggy for our tempo (then use Matchbox for gameplay, still use Nakama for
meta), the realtime protocol is impractical from Rust/wasm, or ops burden on the homelab is too high (then Heroic Cloud
or a small own service for just lobbies).

## 8b. Spike log (findings as they come in)

- **2026-10-24, leaderboards need server code.** Per Nakama's leaderboard docs, "a leaderboard can be created via server-side code at
  startup or within a registered function"; a client can only submit scores to, and list records of, existing leaderboards.
  Practical consequence: even the simplest meta feature on our list needs a server runtime module (JavaScript/TypeScript/Go/Lua)
  deployed alongside Nakama. This strengthens the recommendation to ship **one generic, data-driven module** and keep designers
  out of server code, and it adds a deployer step ("mount the modules directory") that the homelab stack did not have.
- **2026-10-24, E1/E2 measured (native, Wi-Fi, LAN):** REST calls take roughly 8-22 ms (device sign-in, refresh, storage
  read/write). The session token lasts **60 s** (refresh token 3600 s by default), so any long-lived client needs refresh logic. A
  stale storage write returns HTTP 400 with `{"code":9,...,"message":"Storage write rejected - version check failed."}`. Stored JSON
  is **normalized** (same content, different key order and spacing), so save data cannot depend on byte-exact text. The object
  version appears to be derived from the content.
- 2026-10-24, stage 1 endpoint verified: Nakama 3.41.0 healthy, placeholder server key rejected (401), device-id sign-in works.

## 9. What is not verified in this investigation

- Whether **Lightyear, bevy_replicon/renet or matchbox** support our current Bevy version (and the 0.19 target), and their WASM
  transports. The searches did not return a clear answer; check each crate's release notes. Needed for the sync layer in every option.
- Nakama **benchmark numbers** (the page could not be read) and real behaviour at our message sizes.
- The exact **JSON vs protobuf** socket details and whether a hand-written Rust client is as small as estimated.
- **Heroic Cloud / Enterprise / Hiro / Satori pricing and terms**.
- Whether the **latest release's year/changes** affect Postgres requirements (the release list showed none for the last five).

## 10. Sources

- Nakama README (features, license, clients, database): https://raw.githubusercontent.com/heroiclabs/nakama/master/README.md
- Releases (v3.41.0 and earlier): https://github.com/heroiclabs/nakama/releases
- Authoritative multiplayer (tick 1-60, MTU, limits, Enterprise-only migration): https://heroiclabs.com/docs/nakama/concepts/multiplayer/authoritative/
- Session-based multiplayer / FleetManager: https://heroiclabs.com/docs/nakama/concepts/multiplayer/session-based/
- Docker install (Postgres 16.8, ports): https://heroiclabs.com/docs/nakama/getting-started/install/docker/
- Architecture overview (clustering is Enterprise): https://heroiclabs.com/docs/nakama/getting-started/architecture/
- Enterprise vs single node, vendor CCU claim: https://heroiclabs.com/blog/nakama-enterprise/
- Open-source capabilities Q&A (2021, no CockroachDB Enterprise licence needed): https://forum.heroiclabs.com/t/questions-about-the-capabilities-of-nakama-open-source/1783
- Archived Rust client: https://github.com/heroiclabs/nakama-rs
- Realtime socket formats (third-party description): https://apis.io/apis/nakama/nakama-realtime-api/
- Scaling anecdote (single user): https://forum.heroiclabs.com/t/scaling-authoritative-matches-for-200-players-with-high-tick-rate-best-practices-and-configuration-tips/5804
- Fleet manager ecosystem (Gameye, forum on Agones): https://www.gameye.com/docs/guides/integrations/nakama/ and https://forum.heroiclabs.com/t/how-to-connect-dedicated-server-instance-with-nakama-match-instance/5315
