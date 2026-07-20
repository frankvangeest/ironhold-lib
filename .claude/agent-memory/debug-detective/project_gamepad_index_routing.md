---
name: project-gamepad-index-routing
description: How InputMap.gamepad_index routing resolves and its shared-index double-fire fragility (input_translator/targeting/interactable/camera)
metadata:
  type: project
---

Gamepad input is routed per-player/per-camera via `InputMap.gamepad_index: Option<usize>`, resolved through `resolve_gamepad(sorted_slice, index)` (`runtime/input.rs`, `pub(crate)`). Each system builds one `Vec<(Entity,&Gamepad)>` sorted by `Entity::index()` once per frame, then resolves per player. `index.and_then(|i| sorted.get(i))` — so `None` and out-of-range indices are both safe no-ops (no gamepad input, keyboard still works).

**Fragility: no validation guards against two players sharing the same `Some(n)`.** When they do, both resolve to the same physical `Gamepad`, so one button press fires interact/target_next (and drives movement/turn/camera-pitch) for BOTH players simultaneously. `assemble_player_config` warns on unrecognized button *names* but not on duplicate `gamepad_index` across players. This was a pre-existing behavior class for movement (gamepad-routed before the gamepad_controller_input feature); that feature widened the blast radius to interact/target/pitch without adding a duplicate-index check.

**Why:** confirmed during the gamepad_controller_input post-impl review (2026-07-20). `None`-shared is fine; only shared `Some(n)` misbehaves — a designer misconfiguration, not an engine bug.

**How to apply:** if a "both players act on one button" bug is reported in a local-coop scene, check for duplicate `gamepad_index` in the scene's player prefabs first. A cross-player duplicate-index `warn!` (mirroring the [[project_gamepad_button_name_validation]] seam) would be the natural fix if it graduates to a real complaint.

`InputMap` derives only `Deserialize, Debug, Clone` (no `Default`) — so new non-Option String fields can't be silently emptied by a stray `..Default::default()`; every construction site is either serde-defaulted or an explicit literal (`default_input_map()` in `entity_spawner.rs`, test fixtures).
