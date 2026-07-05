---
name: local-coop-input-camera
description: Local co-op Stage 1 hot-path systems — gamepad sort in input_translator, party_camera_follow, view_box_clamp
metadata:
  type: project
---

Local Co-op Split-Screen Stage 1 (added ~2026-07, files: runtime/input.rs, capabilities/camera.rs, capabilities/player.rs).

**input_translator_system (FixedUpdate):** now collects `Query<(Entity,&Gamepad)>` into a Vec and `.sort_by_key(entity.index())` UNCONDITIONALLY every tick, before the CharacterController loop. On single-player web scenes with zero gamepads this is an empty Vec (no heap alloc — Vec::new/collect of 0 items doesn't allocate) + a no-op sort. Effectively free. The sort exists to map `InputMap.gamepad_index: usize` to a stable pad. NOT a per-tick regression.
**Why:** Frank asked if this is a measurable per-tick cost for existing single-player WASM. Answer: no, empty-collect doesn't allocate in Rust.
**How to apply:** Don't flag the gamepad collect/sort. If gamepad_index usage grows or pads connect, the collect is still tiny (2-4 entries). Only concern would be if it moved to per-entity scope.

**party_camera_follow_system (Update, unconditional registration):** queries `Query<(&mut Transform,&mut PartyOrbitCamera)>`. Existing single-player scenes have zero PartyOrbitCamera entities → Bevy 0.18 empty-archetype query iterates nothing, ~free. The O(n^2) pairwise-distance loop is bounded by player count (2-4) → trivial. Per-frame Vec `positions` alloc only happens when a PartyOrbitCamera exists (co-op scenes only), not single-player.

**player_view_box_clamp_system (FixedUpdate, after player_movement):** early-returns via `let Some(..) = view_box.0 else { return }`. ActiveViewBox default/single-player = None (set from scene.max_view_box, None for scenes without it). Early-return before any query iteration → free for existing scenes.

**Gamepad on WASM:** bevy pulls bevy_gilrs by default (workspace bevy has no default-features=false). gilrs wasm backend uses web-sys Gamepad API. `Gamepad` component/query exists identically on both targets — no cfg gap, no panic when no pads granted; query is just empty until browser user-gesture populates navigator.getGamepads(). Graceful.

Zero new deps, zero new render pipelines/materials/shaders — pure ECS/logic. No binary-size impact.
