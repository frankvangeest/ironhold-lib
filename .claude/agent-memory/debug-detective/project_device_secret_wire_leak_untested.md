---
name: project-device-secret-wire-leak-untested
description: Nakama per-game hashed device id (feature/device_id_binding, c445b6e) - no test can tell `{game}:{hash}` from `{game}:{raw secret}` on the wire
metadata:
  type: project
---

Since c445b6e (2026-11-08) the stored device secret and the wire hash are both 32 lowercase hex, so the server's Lua format
check, the contract tests, and `one_secret_gives_two_unrelated_accounts` (which recomputes ids locally via `ns::device_id`)
all still pass if the adapter sent `{game}:{secret}` raw. adapter_flow.rs mock handlers ignore `_body` (all of them), so
nothing asserts the wire id. Pinning it needs an adapter_flow handler that asserts the body equals the golden vector id.

**Why:** the whole point of the feature is "the secret never leaves the client"; format-only checks are blind to it.
**How to apply:** when reviewing changes to client.rs/ns.rs/protocol.rs authenticate paths, check whether a body-level
assertion exists yet; don't accept "contract tests pass" as proof of the hashing. Related: native runner has no
read-back after write (web does), so two concurrent native launches can each use a different secret. [[project-backend-failure-event-semantics]]
