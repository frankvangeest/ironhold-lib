---
name: gamepad-input-pattern
description: Gamepad input is RON-driven via InputMap gamepad_* fields + parse_gamepad_button; resolve_gamepad "sort once per system per frame" shape; test sim couples to Bevy-internal systems
metadata:
  type: project
---

Gamepad/controller input became fully RON-configurable in `feature/gamepad-controller-input` (2026-07-20). Establishes the pattern the dependent "gamepad-routed action-bar slots" backlog item will reuse.

**Shape (all verified sound):**
- `InputMap` gained `gamepad_jump`/`gamepad_run`/`gamepad_interact`/`gamepad_target_next: String` + `gamepad_deadzone: f32`, all `#[serde(default=...)]` matching prior hardcoded literals (South/East/West/North/0.15) — byte-for-byte back-compat.
- `InputMap::parse_gamepad_button(&str) -> Option<GamepadButton>` mirrors `parse_key`; unrecognized name → `None`. Load-time validation warn lives in `assemble_player_config` (entity_spawner.rs), mirroring the keyboard key-name seam. `gamepad_button(name)` is a stringly-typed field dispatch ("jump"/"run"/…), consistent with `key()`.
- `resolve_gamepad<'a>(sorted: &'a [(Entity,&'a Gamepad)], index: Option<usize>) -> Option<&'a Gamepad>` in runtime/input.rs (`pub(crate)`). **Invariant: each system builds ONE sorted-by-Entity::index() slice per frame BEFORE its per-player/per-camera loop, then calls resolve_gamepad many times inside.** Never re-sort per caller. 4 call sites obey this: input_translator_system, camera_orbit_system, interactable_system, tab_targeting_system.

**Spawn-time vs live resolution split:** static per-player values (gamepad_index, gamepad_deadzone) are pre-resolved onto `OrbitCamera` at spawn (like look_left_key etc.); only the live analog stick *value* needs a per-frame `Query<(Entity,&Gamepad)>`. Button-name parsing is done live each frame off the CharacterController's InputMap (not pre-resolved) — consistent with the keyboard path, negligible cost.

**Camera pitch:** right-stick-Y is non-inverted (positive stick → pitch toward max_pitch), matching this codebase's LeftStickY-drives-forward convention. Reuses `OrbitCamera.look_speed` (shared with keyboard look_up/down). Direction pinned by a real direction-asserting test.

**WASM:** safe — no plugin added, relies on DefaultPlugins' GamepadPlugin (browser Gamepad API works); no threading/native calls.

**Bevy-upgrade risk (Minor):** test support (`tests/support/mod.rs`) registers Bevy-internal `gamepad_connection_system`/`gamepad_event_processing_system` (public fns) + ~8 raw gamepad message types manually, because the test app uses MinimalPlugins (no InputPlugin). This couples test infra to Bevy internals — flag on any Bevy upgrade. Harmless to existing ~100 tests: systems no-op without simulated events, no Gamepad entities spawned. See [[bevy_019_upgrade]].
