---
name: nakama-rpc-error-and-hook-facts
description: Nakama 3.41.0 Lua RPC facts probed live - RpcFunc before-hook never runs, error({msg,7}) -> 403, plain error() -> 500 leaking module path, ids lowercased, http_key path has no vars
metadata:
  type: project
---

Probed against the contract stack (backend/docker-compose.yml, Nakama 3.41.0) on 2026-11-07 while reviewing feature/lobby_rpc_allowlist:

- `nk.register_req_before(fn, "RpcFunc")` registers without error but never runs on `POST /v2/rpc/{id}` (an unguarded RPC still answered 200). Only the realtime `Rpc` rt-before hook (`*rtapi.envelope_rpc`) shows up in the "Registered Lua runtime Before" log.
- Inside a Lua RPC, `error({ msg, 7 })` -> HTTP 403 `{"code":7,"message":msg}`. A plain `error("boom")` -> HTTP 500 code 13 whose message is `/nakama/data/modules/<file>.lua:<line>: boom`, so internal paths and error text reach the client.
- RPC ids are case-insensitive for callers (`/v2/rpc/IRONHOLD_LOBBY_PROBE` hits `ironhold_lobby_probe`).
- `?http_key=` calls have `context.vars` empty; a wrong key is 401 (code 16) before any Lua runs, so `assert_ne!(status, 200)` cannot tell the guard from a bad key.
- `context.vars` comes only from the token: device sign-in vars are overwritten by the before-hook, and a refresh with `vars` keeps the original game.

**Why:** these decide whether a game guard in a Lua RPC holds and whether a test that only checks "not 200" proves anything.
**How to apply:** for an RPC guard test, assert 403 plus the message; wrap game RPC bodies so their own errors do not leak paths. Probe recipe: copy the modules to a scratch folder, add a zz_probe.lua, start the stack under its own `-p` name and port, then `down -v`. See [[backend-dev-stack-singleton-and-scan]].
