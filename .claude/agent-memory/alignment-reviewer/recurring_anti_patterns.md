---
name: Recurring data-driven anti-patterns in ironhold_core
description: Repeated patterns that block designer reachability without recompile; consult when reviewing for alignment with the project promise
type: project
---

Anti-patterns that recur in this codebase. Snapshot updated during a fresh anti-pattern audit on 2026-05-03.

## Status of historic blockers (2026-05-01 → 2026-05-03)

Resolved since last review:
- `player.rs::player_movement_system` no longer pushes `Action::PlaySound` directly — now emits `GameEvent::Trigger("player.jumped")`.
- `material_factory.rs` no longer hardcodes `"shared/terrain/{grass,rock,dirt,snow}.png"` fallback paths. Layers come from `terrain_def.layers` and missing slots load `Handle::default()`.
- `terrain.rs::poll_terrain_generation_system` reads `config.uv_scale` instead of hardcoded `10.0`.
- NPC magic numbers (`alerted_duration 0.3`, `waypoint_reach_radius 0.5`, `eye_height 0.9`, `drag 0.8`) are all now schema fields with named `default_npc_*` constants.
- Player movement parameters (`walk_speed`, `run_speed`, `rot_speed`, `jump`) flow through `MovementConfig` for both primitive and GLB players (`spawn_player_entity`).
- `Action::Spawn` now has `position`, `spawn_point`, and `yaw_deg` fields with proper `#[serde(default)]`.
- `spin.rs` is fully removed (file deleted from disk and `mod.rs`).
- No capability touches `ResMut<ActionQueue>` — only the four interpreter systems do.

## Currently active anti-patterns

**1. Hardcoded fallback asset path.**
`scene_loader.rs:510` falls back to `"prefabs/animation/player_policy.ron"` when a player prefab omits `animation_policy`. All shipped projects set the field, so this is dead code in practice but remains a magic constant in core. The schema field is `Option<String>` — the runtime should warn and skip animation rather than substituting a hardcoded path.

**2. Embedded shader strings via `include_str!`.**
- `terrain.rs::setup_terrain_shader`: embeds `assets/shared/shaders/terrain.wgsl`.
- `custom_material.rs::setup_custom_material_fallback_shader`: embeds `assets/shared/shaders/custom_material_default.wgsl`.
This is a deliberate WebGPU-bootstrapping exception (avoids a race between asset loader and pipeline compilation on WASM). Acceptable for fallback/default shaders, but a designer cannot replace `terrain.wgsl` without recompiling. Document the constraint or expose terrain shader path via `TerrainConfigV2`.

**3. Hardcoded mouse-button input contracts.**
- `input.rs::input_translator_system` hardcodes `MouseButton::Left` for strafe-mode toggle.
- `camera.rs::camera_orbit_system` hardcodes `MouseButton::Left/Right` for orbit/character-rotate.
- `flycam.rs` hardcodes WASD/QE/Space/Ctrl/Shift/LMB/RMB and is documented as "fixed, not configurable". The flycam doc-comment is honest, but it still violates the data-driven mandate when a designer wants different bindings.

**4. Camera pitch clamps and physics constants in core.**
- `camera.rs:61`: pitch clamp `0.1, 1.5` is hardcoded — designers can't widen the orbit pitch range.
- `entity_spawner.rs:244`: `ground_cast_length: 0.3` hardcoded for player ground-detection.
- `entity_spawner.rs:270` / `scene_loader.rs:449`: `Damping { 0.5, 0.5 }` hardcoded for player and NPC capsules.
- `entity_spawner.rs:293`: orbit camera initial pitch `0.5`, yaw `0.0` hardcoded — non-zero pitch is invisible to the designer until they realise the spawn camera angle is offset.

**5. Player movement still has minor magic numbers.**
- `player.rs:140-141`: drag `0.8` hardcoded (was previously called out; not promoted to MovementConfig).
- `player.rs:121`: rotation speed multiplier reads `controller.rot_speed` (good) but the threshold `move_vec.length_squared() > 0.1` (line 125) is hardcoded.

**6. NPC and physics still have small thresholds.**
- `npc.rs:220`: interact-leave hysteresis `* 1.5` of approach_distance — not a schema field.
- `npc.rs:235`: return-home distance `< 0.5` hardcoded.
- `interactable.rs`: distance compare uses `<= radius` directly (correct), but no hysteresis — interactable hint flickers if player straddles the boundary.

**7. Things the codebase gets right consistently (do not regress).**
- `collectible_system`, `trigger_zone_system`, `interactable_system`, `npc_behavior_system` all emit `GameEvent::Trigger` with documented namespaced names — never push to `ActionQueue`.
- The `Message → Interpreter → Action → Executor` flow is now respected by 100% of capabilities (no direct ActionQueue access in `capabilities/`).
- Schema files have proper `#[serde(default)]` on optional fields with named default fns.
- `Action` enum is fully documented; every variant has a corresponding `match` arm in `action_executor_system`.
- `pending.unwrap()` in `project_loader.rs:102` and `inspector.rs` are guarded — no fallible unwraps reachable from designer-authored RON.
- `model_spawner.rs::spawn_instance` reads transform fixes from the merged catalog rather than hardcoding any path.
- Asset paths in `material_factory.rs::build_standard` flow through `std_def.*_texture` fields — no engine-side asset-layout knowledge.

## Audit prompt for future reviews

When a new system is added, ask: "If a designer wants to add a sound effect / score change / state transition / threshold tweak triggered by this system's event, is that achievable in RON without a recompile?" If it requires touching a magic number in Rust, that number belongs in a schema struct as `Option<f32>` with a `default_*` named function.
