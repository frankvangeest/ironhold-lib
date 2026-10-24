---
name: nakama-spike-facts
description: Verified Nakama 3.x defaults and ewebsock native-loop facts that distort latency measurements or break the spike plan's E3/E4/E5 assumptions
metadata:
  type: project
---

Facts verified 2026-10-24 (vendor docs page and ewebsock main source) during the plan review of `planning/features/nakama_backend_spike.md`:

- Nakama defaults: `session.token_expiry_sec` is 60, `refresh_token_expiry_sec` is 3600, `socket.max_message_size_bytes` is 4096 (inbound per message), `socket.outgoing_queue_size` is **16** (a slow reader is disconnected), `ping_period_ms` is 15000, `pong_wait_ms` is 25000, `write_wait_ms` is 5000, and `matchmaker.interval_sec` is 15 (so a match forms in up to about 15 s).
- Leaderboards can only be created by server runtime code, so a "REST-only, no server module" deployment cannot run E3. Authoritative leaderboards reject client writes.
- Relayed matches accept an optional `name` (a deterministic match id). The docs never mention labels for them, so a labelled lobby list probably needs an authoritative match (server code), storage objects or the matchmaker. Relayed data goes to all *other* presences by default, and a presence subset can be targeted. The match ends when it is empty.
- ewebsock native (`native_tungstenite.rs`) runs one loop: it sends at most ONE queued message, reads with `read_timeout` (default 10 ms), then yields. That adds up to 10 ms of send latency and caps the send rate at about 100 msg/s while the socket is idle. It never calls `set_nodelay`, so Nagle is on.

**Why:** these facts turn "WebSocket relay is too slow" into an artefact of the client or the configuration, not a property of Nakama.
**How to apply:** any future Nakama, ehttp or ewebsock work, the backend-abstraction plan, or the Rust client crate. Re-check them when the Nakama version changes. See [[determinism-networking]].
