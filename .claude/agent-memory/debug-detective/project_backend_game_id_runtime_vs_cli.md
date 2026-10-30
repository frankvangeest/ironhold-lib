---
name: backend-game-id-runtime-vs-cli
description: BackendConfig::problems() never checks the effective game_id (project_id fallback); only CLI backend_checks does, so the runtime first fails inside the Nakama factory (GameNs::new), after side effects
metadata:
  type: project
---

`BackendConfig::problems()` (schema/backend.rs) validates `game_id` only when it is set explicitly. The fallback to
`.project.ron`'s `project_id` (or `""` via `effective_game_id(..).unwrap_or_default()` in runtime/backend.rs build_adapter)
is checked only by the CLI (`backend_checks.rs`, "no game_id here and no project_id"). At runtime the first check is
`GameNs::new` inside `NakamaBackend::new`, i.e. after the factory already ran its side effects (observed 2026-10-30 on
feature/public_backend_demo: the lazy factory calls the `device_uuid` source, which writes localStorage / ~/.ironhold/device_id,
before `NakamaBackend::new` fails on the namespace).

**Why:** runtime and CLI disagree on what "valid" means for the namespace, so anything ordered before `GameNs::new` in the
adapter path happens for a config that can never connect.

**How to apply:** when reviewing backend factory/adapter changes, check what side effects run before `GameNs::new` /
leaderboard checks; when a "no device id for offline projects" claim is made, test the no-game_id + no-project_id case.
Related: [[schema-validate-errors-are-nonfatal]] (different mechanism: backend.ron problems() IS fatal, it goes offline).
