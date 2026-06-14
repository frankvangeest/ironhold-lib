---
name: npc-glb-actor-capsule-pattern
description: components.npc on GLB Actor prefabs — capsule collider dimensions are hardcoded in entity_spawner.rs (not designer-tunable); NpcDef must stay non-deny_unknown_fields; LocomotionState wiring in npc.rs
metadata:
  type: project
---

`components.npc` (NpcDef in schema/catalog.rs) works on BOTH Primitive and GLB Actor/Prop prefabs. The GLB path lives in `runtime/scene_manager/entity_spawner.rs::spawn_prefab_instance` under `if let Some(npc_def) = &prefab.components.npc`.

**Known hardcoded-but-accepted constant**: the NPC capsule collider radius (0.35 m) and total height (1.6 m) are hardcoded in `spawn_prefab_instance`, NOT read from NpcDef. Comment says "Keeps GLB enemy colliders out of each other without requiring designer-specified sizes." This is a real designer-reachability gap (tall/short creatures all get the same capsule) but is currently a WARNING not BLOCKING — every gameplay-affecting NpcDef field (radii, speeds, fov, eye_height, damping, drag) IS data-driven; only the physical capsule body dimensions are not. If a designer adds a very large or very small creature, the capsule will mismatch the model. Suggested fix when revisited: add optional `collider_radius` / `collider_height` to NpcDef with the current values as defaults (mirrors MovementConfig which already does this for players).

**NpcDef must NOT have `#[serde(deny_unknown_fields)]`** — it currently does not (catalog.rs ~line 1015). Adding it is fine but be deliberate; the struct has many `#[serde(default)]` fields.

**LocomotionState wiring**: `capabilities/npc.rs::npc_behavior_system` queries `Option<&mut LocomotionState>` and sets `moving`/`running`/`is_grounded` each tick so GLB NPC animation (idle/walk/run via animation policy) responds to movement. Primitive NPCs have no AnimationPolicyComponent so the Option is None for them. The component is inserted by the `animation_policy` branch in spawn_prefab_instance, NOT the npc branch — so an NPC without an `animation_policy` gets no locomotion-driven animation (correct, since it has no clips).

**Pipeline correctness**: npc_behavior_system emits `GameEvent::Trigger("npc.player_reached:{id}")` etc. via MessageWriter — it does NOT push to ActionQueue. This is the correct capability pattern. Behavior files (.behavior.ron) react to these events through the FSM and push actions through the normal pipeline. When reviewing NPC changes, confirm the capability still only emits GameEvent and never gains ResMut<ActionQueue>.

**Designer reachability of a new GLB enemy is complete with zero Rust**: assets.ron model key + prefabs.ron Actor prefab with components.npc + animation_policy + behavior + stat_templates + scene instances. The two snake/spider enemies in 3rd_person_game_demo are the canonical example.
