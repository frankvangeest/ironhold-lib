---
name: npc-glb-actor-capsule-pattern
description: components.npc on GLB Actor prefabs — capsule collider dimensions are hardcoded in entity_spawner.rs (not designer-tunable); NpcDef must stay non-deny_unknown_fields; LocomotionState wiring in npc.rs
metadata:
  type: project
---

`components.npc` (NpcDef in schema/catalog.rs) works on BOTH Primitive and GLB Actor/Prop prefabs. The GLB path lives in `runtime/scene_manager/entity_spawner.rs::spawn_prefab_instance` under `if let Some(npc_def) = &prefab.components.npc`.

**Capsule dimensions are now data-driven (RESOLVED ~2026-06-14):** `NpcDef.collider_radius: Option<f32>` and `collider_height: Option<f32>` (catalog.rs ~1078/1082, both `#[serde(default)]`) are read in `spawn_prefab_instance` via `.unwrap_or(0.35)` / `.unwrap_or(1.6)` (entity_spawner.rs ~171-172). Mirrors the `MovementConfig.collider_radius/height` pattern exactly. Defaults preserved so existing prefabs are unaffected. The prior WARNING about humanoid-capsule mismatch on large/small creatures is closed. NOTE (still open as a soft warning): no test project actually sets these fields yet — the snake/spider creatures in 3rd_person_game_demo are the natural reference case (snake is long+low, mismatches the default capsule).

**NpcDef must NOT have `#[serde(deny_unknown_fields)]`** — it currently does not (catalog.rs ~line 1015). Adding it is fine but be deliberate; the struct has many `#[serde(default)]` fields.

**LocomotionState wiring**: `capabilities/npc.rs::npc_behavior_system` queries `Option<&mut LocomotionState>` and sets `moving`/`running`/`is_grounded` each tick so GLB NPC animation (idle/walk/run via animation policy) responds to movement. Primitive NPCs have no AnimationPolicyComponent so the Option is None for them. The component is inserted by the `animation_policy` branch in spawn_prefab_instance, NOT the npc branch — so an NPC without an `animation_policy` gets no locomotion-driven animation (correct, since it has no clips).

**Pipeline correctness**: npc_behavior_system emits `GameEvent::Trigger("npc.player_reached:{id}")` etc. via MessageWriter — it does NOT push to ActionQueue. This is the correct capability pattern. Behavior files (.behavior.ron) react to these events through the FSM and push actions through the normal pipeline. When reviewing NPC changes, confirm the capability still only emits GameEvent and never gains ResMut<ActionQueue>.

**Designer reachability of a new GLB enemy is complete with zero Rust**: assets.ron model key + prefabs.ron Actor prefab with components.npc + animation_policy + behavior + stat_templates + scene instances. The two snake/spider enemies in 3rd_person_game_demo are the canonical example.
