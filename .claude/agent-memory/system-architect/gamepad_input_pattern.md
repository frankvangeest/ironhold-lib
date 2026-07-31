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

**Positional-index fragility (load-bearing for any runtime pad binding):** `gamepad_index` is a *position* in the per-frame sorted slice, not an identity. A pad disconnecting shifts every higher index, silently re-routing live players to different pads. Tolerable while indices are RON-authored (designer can retry); becomes a correctness problem the moment an index is assigned at *runtime* (gamepad hot-join). If that lands, push for a resolved `Entity`-based runtime binding on `CharacterController`/`OrbitCamera` with `gamepad_index` demoted to an authored seed.

**Where the phantom-duplicate-pad quirk is documented:** `docs/20_data_formats.md` (InputMap troubleshooting callout, ~line 1845) — NOT in any `CLAUDE.md`, despite plans citing it there. Key property: the dead duplicate reports **zero for every axis/button, always**. So "require live signal" filters it by construction; the mitigation that actually matters is `just_pressed` edge semantics, not a separate liveness gate.

**Key-bindings merge has TWO insert sites**, not one: `project_loader.rs` inserts `ProjectKeyBindings` + `LoadedKeyBindings` in both the inline-config path (~line 116) and the external-files path (~line 250); `scene_loader.rs` rebuilds `LoadedKeyBindings` from the `ProjectKeyBindings` base each Replace load. Any new sibling binding map must replicate the base-layer resource *and* all three sites or bindings bleed across scenes on one load path only.

**Gamepad hot-join (`gamepad_hot_join.md`, reviewed 2026-07-29, feature branch uncommitted) — the ONE structural weakness to remember:** pad identity travels **out-of-band** from the event. Detection (`unclaimed_gamepad_trigger_system`, Update, `.before(message_interpreter_system)`) emits a plain `UiEvent::ButtonPressed(trigger)` — which carries no pad — and separately writes `PendingJoinGamepad(Option<Entity>)`, which `Action::JoinPlayer`'s executor arm `take()`s. Because the event and the identity are decoupled, pairing is only correct when exactly ONE join-producing event exists per frame. Two consequences that recur in any future variant:
- Emitting an event for every match while capturing only one pad **does not** "drop the second press" — the interpreter loops per `UiEvent` (`message_interpreter.rs:19`) and the executor's `queued_hot_joins` counter happily assigns slot N+1, so you get a second real player with no pad bound. "At most one captured pad per frame" ≠ "at most one join per frame".
- `global_input_system` and `unclaimed_gamepad_trigger_system` have **no relative ordering** (both merely `.before` the interpreter), so a same-frame keyboard+gamepad join has nondeterministic `UiEvent` append order → the pad can be bound to the keyboard-triggered joiner.
The real fix (logged as follow-up, not done) is intrinsic pairing: a `UiEvent` variant carrying the source pad. Recommend that before building any second consumer of `PendingJoinGamepad`.

Also: `PendingJoinGamepad` is frame-scoped (reset unconditionally at the top of detection), which creates an **undocumented RON authoring constraint** — `JoinPlayer` must be produced by a rule reacting *synchronously* to `ui.button_pressed:<trigger>`. Routing it through `EmitEvent` → second rule, or a delayed event, still joins but silently binds no pad (executor-emitted GameEvents are read by the interpreter next frame).

**`*_gamepad_bindings` are NOT the gamepad twin of `*_key_bindings`** despite the mirrored names/merge logic: a match only fires on an **unclaimed** pad, so there is no way to author a global gamepad trigger (e.g. pause) from an already-joined player's pad. Naming implies symmetry that does not exist.

**Bevy-upgrade risk (Minor):** test support (`tests/support/mod.rs`) registers Bevy-internal `gamepad_connection_system`/`gamepad_event_processing_system` (public fns) + ~8 raw gamepad message types manually, because the test app uses MinimalPlugins (no InputPlugin). This couples test infra to Bevy internals — flag on any Bevy upgrade. Harmless to existing ~100 tests: systems no-op without simulated events, no Gamepad entities spawned. See [[bevy_019_upgrade]].
