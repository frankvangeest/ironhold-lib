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

**The unenforced premise now has a dependent.** As of the gamepad-routed action-bar-slots feature (2026-07-31 review), both `scene_loader.rs::warn_same_player_gamepad_duplicate_slots` and `ironhold_cli validate`'s matching check key collisions by `(owner_player.unwrap_or(0), GamepadButton)` and *deliberately do not flag* two different players sharing a button name — justified in-comment as "each has their own physical pad." That justification is exactly the premise nothing validates. Shared `Some(n)` therefore produces a real same-press double-fire of two players' action-bar slots that both collision checks report as clean.

**Second, distinct hazard: `resolve_gamepad` is positional, not identity-stable.** `sorted_gamepads` is rebuilt from live `Gamepad` entities every frame, so a mid-session disconnect *re-indexes* everyone below it. With P1 on 0 and P2 on 1, unplugging pad 0 makes P1's `gamepad_index: 0` resolve to P2's physical controller (P2 now presses P1's buttons) while P2's index 1 goes out of range and silently stops responding. Panic-free but silent. Reach for this when a local-coop gamepad bug is described as "started acting weird after I unplugged/replugged a controller."

`InputMap` derives only `Deserialize, Debug, Clone` (no `Default`) — so new non-Option String fields can't be silently emptied by a stray `..Default::default()`; every construction site is either serde-defaulted or an explicit literal (`default_input_map()` in `entity_spawner.rs`, test fixtures).
